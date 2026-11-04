//! Reads the cells of an Excel workbook (.xlsx/.xlsm): a zip with one XML file per sheet.
//! Only what the church's schedules need: texts, numbers and the saved results of formulas.
//! Nothing in the workbook is run (no macros, no formulas).

use anyhow::{Context, Result, bail};
use chrono::NaiveDate;
use std::collections::BTreeMap;
use std::io::Read;

#[derive(Debug, Clone, PartialEq)]
pub enum Cell {
    Text(String),
    Number(f64),
}

#[derive(Debug, Default)]
pub struct Sheet {
    pub name: String,
    /// By (row, column), both counted from 1; empty cells are absent.
    cells: BTreeMap<(u32, u32), Cell>,
}

impl Sheet {
    pub fn cell(&self, row: u32, col: u32) -> Option<&Cell> {
        self.cells.get(&(row, col))
    }

    /// The cell as text, trimmed; a whole number without ".0", as Excel shows it.
    pub fn text(&self, row: u32, col: u32) -> String {
        match self.cell(row, col) {
            Some(Cell::Text(t)) => t.trim().to_string(),
            Some(Cell::Number(n)) if n.fract() == 0.0 && n.abs() < 1e15 => format!("{}", *n as i64),
            Some(Cell::Number(n)) => n.to_string(),
            None => String::new(),
        }
    }

    pub fn last_row(&self) -> u32 {
        self.cells.keys().next_back().map_or(0, |(r, _)| *r)
    }

    /// The filled cells of one row, from left to right.
    pub fn row(&self, row: u32) -> impl Iterator<Item = (u32, &Cell)> {
        self.cells.range((row, 0)..(row + 1, 0)).map(|((_, c), cell)| (*c, cell))
    }

    /// The first cell (row by row) whose text is `text`, ignoring case: (row, column).
    pub fn find(&self, text: &str) -> Option<(u32, u32)> {
        self.cells.iter().find(|(_, c)| matches!(c, Cell::Text(t) if t.trim().eq_ignore_ascii_case(text))).map(|(k, _)| *k)
    }

    /// The column in `row` whose text is `text`, ignoring case.
    pub fn find_in_row(&self, row: u32, text: &str) -> Option<u32> {
        self.row(row).find(|(_, c)| matches!(c, Cell::Text(t) if t.trim().eq_ignore_ascii_case(text))).map(|(col, _)| col)
    }
}

#[derive(Debug, Default)]
pub struct Workbook {
    pub sheets: Vec<Sheet>,
    /// Dates count from 1904 instead of 1900 (old Mac workbooks).
    date1904: bool,
}

impl Workbook {
    pub fn sheet(&self, name: &str) -> Option<&Sheet> {
        self.sheets.iter().find(|s| s.name.trim().eq_ignore_ascii_case(name))
    }

    /// The date of an Excel serial number (days since 1900, with Excel's own leap day in 1900;
    /// the time of day is ignored).
    pub fn date(&self, serial: f64) -> Option<NaiveDate> {
        if !(1.0..2_958_466.0).contains(&serial) {
            return None;
        }
        let base = if self.date1904 { NaiveDate::from_ymd_opt(1904, 1, 1)? } else { NaiveDate::from_ymd_opt(1899, 12, 30)? };
        base.checked_add_days(chrono::Days::new(serial.floor() as u64))
    }
}

const NS_REL: &str = "http://schemas.openxmlformats.org/officeDocument/2006/relationships";

/// Reads a workbook from the bytes of its file.
pub fn read(bytes: &[u8]) -> Result<Workbook> {
    let files = unzip(bytes).context("Dit is geen Excel-bestand (.xlsx of .xlsm)")?;
    let file = |name: &str| -> Result<String> {
        let data = files.get(name).with_context(|| format!("{name} ontbreekt in het Excel-bestand"))?;
        Ok(String::from_utf8_lossy(data).into_owned())
    };
    let workbook = file("xl/workbook.xml")?;
    let doc = roxmltree::Document::parse(&workbook).context("xl/workbook.xml")?;
    let date1904 = doc
        .descendants()
        .find(|n| n.has_tag_name("workbookPr"))
        .and_then(|n| n.attribute("date1904"))
        .is_some_and(|v| v == "1" || v == "true");
    let rels = file("xl/_rels/workbook.xml.rels")?;
    let rels_doc = roxmltree::Document::parse(&rels).context("workbook.xml.rels")?;
    let targets: BTreeMap<&str, &str> = rels_doc
        .descendants()
        .filter(|n| n.has_tag_name("Relationship"))
        .filter_map(|n| Some((n.attribute("Id")?, n.attribute("Target")?)))
        .collect();
    let strings = match files.get("xl/sharedStrings.xml") {
        Some(data) => shared_strings(&String::from_utf8_lossy(data))?,
        None => Vec::new(),
    };
    let mut sheets = Vec::new();
    for node in doc.descendants().filter(|n| n.has_tag_name("sheet")) {
        let name = node.attribute("name").unwrap_or_default().to_string();
        let Some(target) = node.attribute((NS_REL, "id")).and_then(|id| targets.get(id)) else { continue };
        // Targets are relative to xl/, or absolute within the package.
        let path = match target.strip_prefix('/') {
            Some(abs) => abs.to_string(),
            None => format!("xl/{target}"),
        };
        let Ok(xml) = file(&path) else { continue };
        let cells = sheet_cells(&xml, &strings).with_context(|| format!("tabblad {name}"))?;
        sheets.push(Sheet { name, cells });
    }
    if sheets.is_empty() {
        bail!("Geen tabbladen gevonden in het Excel-bestand");
    }
    Ok(Workbook { sheets, date1904 })
}

/// The texts that cells refer to by number. A text can be split in runs with their own
/// formatting; the phonetic guides (rPh) are not part of it.
fn shared_strings(xml: &str) -> Result<Vec<String>> {
    let doc = roxmltree::Document::parse(xml).context("xl/sharedStrings.xml")?;
    Ok(doc
        .root_element()
        .children()
        .filter(|n| n.has_tag_name("si"))
        .map(|si| {
            si.descendants()
                .filter(|n| n.has_tag_name("t") && !n.ancestors().any(|a| a.has_tag_name("rPh")))
                .filter_map(|t| t.text())
                .collect()
        })
        .collect())
}

/// "AB12" → (row 12, column 28).
fn cell_ref(r: &str) -> Option<(u32, u32)> {
    let split = r.find(|c: char| c.is_ascii_digit())?;
    let (letters, digits) = r.split_at(split);
    if letters.is_empty() || !letters.chars().all(|c| c.is_ascii_alphabetic()) {
        return None;
    }
    let col = letters.chars().fold(0u32, |acc, c| acc * 26 + (c.to_ascii_uppercase() as u32 - 'A' as u32 + 1));
    Some((digits.parse().ok()?, col))
}

fn sheet_cells(xml: &str, strings: &[String]) -> Result<BTreeMap<(u32, u32), Cell>> {
    let doc = roxmltree::Document::parse(xml)?;
    let mut cells = BTreeMap::new();
    let Some(data) = doc.descendants().find(|n| n.has_tag_name("sheetData")) else { return Ok(cells) };
    let mut row_no = 0;
    for row in data.children().filter(|n| n.has_tag_name("row")) {
        // Row and cell numbers may be left out: then they follow the previous one.
        row_no = row.attribute("r").and_then(|r| r.parse().ok()).unwrap_or(row_no + 1);
        let mut col_no = 0;
        for c in row.children().filter(|n| n.has_tag_name("c")) {
            let (r, col) = c.attribute("r").and_then(cell_ref).unwrap_or((row_no, col_no + 1));
            col_no = col;
            let value = c.children().find(|n| n.has_tag_name("v")).and_then(|v| v.text()).unwrap_or_default();
            let cell = match c.attribute("t").unwrap_or("n") {
                "s" => value.trim().parse::<usize>().ok().and_then(|i| strings.get(i)).map(|s| Cell::Text(s.clone())),
                "inlineStr" => {
                    let text: String = c.descendants().filter(|n| n.has_tag_name("t")).filter_map(|t| t.text()).collect();
                    Some(Cell::Text(text))
                }
                "str" | "d" => Some(Cell::Text(value.to_string())),
                // An error such as #N/A: as good as empty.
                "e" => None,
                _ => value.trim().parse::<f64>().ok().map(Cell::Number),
            };
            match cell {
                Some(Cell::Text(t)) if t.trim().is_empty() => {}
                Some(cell) => {
                    cells.insert((r, col), cell);
                }
                None => {}
            }
        }
    }
    Ok(cells)
}

/// Workbooks are small; anything bigger than this inside is not a workbook.
const MAX_FILE: u64 = 64 * 1024 * 1024;

fn u16_at(b: &[u8], at: usize) -> Result<u16> {
    let s = b.get(at..at + 2).context("zip te kort")?;
    Ok(u16::from_le_bytes([s[0], s[1]]))
}

fn u32_at(b: &[u8], at: usize) -> Result<u32> {
    let s = b.get(at..at + 4).context("zip te kort")?;
    Ok(u32::from_le_bytes([s[0], s[1], s[2], s[3]]))
}

/// The XML files in a zip archive, by name. Only what workbooks use: stored or deflated
/// entries, no encryption, no zip64.
fn unzip(b: &[u8]) -> Result<BTreeMap<String, Vec<u8>>> {
    // The end-of-central-directory record, at the end behind an optional comment.
    let lowest = b.len().saturating_sub(22 + 65_535);
    let end = (lowest..=b.len().saturating_sub(22))
        .rev()
        .find(|&i| b[i..].starts_with(&[0x50, 0x4b, 0x05, 0x06]))
        .context("geen zip-bestand")?;
    let entries = u16_at(b, end + 10)? as usize;
    let mut at = u32_at(b, end + 16)? as usize;
    let mut files = BTreeMap::new();
    for _ in 0..entries {
        if u32_at(b, at)? != 0x0201_4b50 {
            bail!("zip-inhoudsopgave beschadigd");
        }
        let flags = u16_at(b, at + 8)?;
        let method = u16_at(b, at + 10)?;
        let packed = u32_at(b, at + 20)?;
        let size = u32_at(b, at + 24)?;
        let (name_len, extra_len, comment_len) = (u16_at(b, at + 28)? as usize, u16_at(b, at + 30)? as usize, u16_at(b, at + 32)? as usize);
        let local = u32_at(b, at + 42)? as usize;
        let name = String::from_utf8_lossy(b.get(at + 46..at + 46 + name_len).context("zip te kort")?).into_owned();
        at += 46 + name_len + extra_len + comment_len;
        if !(name.ends_with(".xml") || name.ends_with(".rels")) {
            continue;
        }
        if flags & 1 != 0 {
            bail!("het Excel-bestand is beveiligd met een wachtwoord");
        }
        if packed == u32::MAX || size == u32::MAX || local == u32::MAX as usize {
            bail!("het Excel-bestand is te groot");
        }
        if u32_at(b, local)? != 0x0403_4b50 {
            bail!("zip-bestand beschadigd bij {name}");
        }
        let start = local + 30 + u16_at(b, local + 26)? as usize + u16_at(b, local + 28)? as usize;
        let raw = b.get(start..start + packed as usize).with_context(|| format!("zip te kort bij {name}"))?;
        let data = match method {
            0 => raw.to_vec(),
            8 => {
                let mut out = Vec::with_capacity(size as usize);
                flate2::read::DeflateDecoder::new(raw).take(MAX_FILE).read_to_end(&mut out).with_context(|| format!("uitpakken van {name}"))?;
                out
            }
            m => bail!("onbekende zip-compressie {m} bij {name}"),
        };
        files.insert(name, data);
    }
    Ok(files)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::io::Write;

    /// A small zip like Excel writes it (deflated entries), for the tests.
    pub(crate) fn zip(files: &[(&str, &str)]) -> Vec<u8> {
        let mut out = Vec::new();
        let mut central = Vec::new();
        for (name, content) in files {
            let mut enc = flate2::write::DeflateEncoder::new(Vec::new(), flate2::Compression::default());
            enc.write_all(content.as_bytes()).unwrap();
            let packed = enc.finish().unwrap();
            let offset = out.len() as u32;
            let header = |sig: u32, central: bool| {
                let mut h = Vec::new();
                h.extend(sig.to_le_bytes());
                if central {
                    h.extend(20u16.to_le_bytes());
                }
                h.extend(20u16.to_le_bytes());
                h.extend(0u16.to_le_bytes()); // flags
                h.extend(8u16.to_le_bytes()); // deflate
                h.extend(0u32.to_le_bytes()); // time and date
                h.extend(0u32.to_le_bytes()); // crc (not checked)
                h.extend((packed.len() as u32).to_le_bytes());
                h.extend((content.len() as u32).to_le_bytes());
                h.extend((name.len() as u16).to_le_bytes());
                h.extend(0u16.to_le_bytes()); // extra
                if central {
                    h.extend(0u16.to_le_bytes()); // comment
                    h.extend(0u16.to_le_bytes()); // disk
                    h.extend(0u16.to_le_bytes()); // internal attributes
                    h.extend(0u32.to_le_bytes()); // external attributes
                    h.extend(offset.to_le_bytes());
                }
                h.extend(name.as_bytes());
                h
            };
            out.extend(header(0x0403_4b50, false));
            out.extend(&packed);
            central.extend(header(0x0201_4b50, true));
        }
        let cd_offset = out.len() as u32;
        out.extend(&central);
        out.extend(0x0605_4b50u32.to_le_bytes());
        out.extend([0u8; 4]);
        out.extend((files.len() as u16).to_le_bytes());
        out.extend((files.len() as u16).to_le_bytes());
        out.extend((central.len() as u32).to_le_bytes());
        out.extend(cd_offset.to_le_bytes());
        out.extend(0u16.to_le_bytes());
        out
    }

    /// A workbook with the given sheets; each sheet's rows are lists of (cell, value), where
    /// a value starting with '#' is a number and anything else a shared text.
    pub(crate) fn workbook(sheets: &[(&str, &[&[(&str, &str)]])]) -> Vec<u8> {
        let mut strings: Vec<String> = Vec::new();
        let mut files: Vec<(String, String)> = Vec::new();
        let mut list = String::new();
        let mut rels = String::new();
        for (i, (name, rows)) in sheets.iter().enumerate() {
            let mut data = String::new();
            for row in rows.iter() {
                let r: String = row[0].0.chars().filter(|c| c.is_ascii_digit()).collect();
                data.push_str(&format!("<row r=\"{r}\">"));
                for (cell, value) in row.iter() {
                    match value.strip_prefix('#') {
                        Some(n) => data.push_str(&format!("<c r=\"{cell}\"><v>{n}</v></c>")),
                        None => {
                            let k = strings.iter().position(|s| s == value).unwrap_or_else(|| {
                                strings.push(value.to_string());
                                strings.len() - 1
                            });
                            data.push_str(&format!("<c r=\"{cell}\" t=\"s\"><v>{k}</v></c>"));
                        }
                    }
                }
                data.push_str("</row>");
            }
            files.push((format!("xl/worksheets/sheet{}.xml", i + 1), format!("<worksheet xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\"><sheetData>{data}</sheetData></worksheet>")));
            list.push_str(&format!("<sheet name=\"{name}\" sheetId=\"{}\" r:id=\"rId{}\"/>", i + 1, i + 1));
            rels.push_str(&format!("<Relationship Id=\"rId{}\" Type=\"worksheet\" Target=\"worksheets/sheet{}.xml\"/>", i + 1, i + 1));
        }
        let sst: String = strings.iter().map(|s| format!("<si><t xml:space=\"preserve\">{}</t></si>", html_escape::encode_text(s))).collect();
        files.push(("xl/sharedStrings.xml".into(), format!("<sst xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\">{sst}</sst>")));
        files.push((
            "xl/workbook.xml".into(),
            format!("<workbook xmlns=\"http://schemas.openxmlformats.org/spreadsheetml/2006/main\" xmlns:r=\"{NS_REL}\"><sheets>{list}</sheets></workbook>"),
        ));
        files.push(("xl/_rels/workbook.xml.rels".into(), format!("<Relationships xmlns=\"http://schemas.openxmlformats.org/package/2006/relationships\">{rels}</Relationships>")));
        zip(&files.iter().map(|(n, c)| (n.as_str(), c.as_str())).collect::<Vec<_>>())
    }

    #[test]
    fn reads_cells_of_a_workbook() {
        let rows: &[&[(&str, &str)]] = &[&[("A1", "Datum"), ("B1", "Omschrijving")], &[("A3", "#46327"), ("B3", "Heilig Avondmaal"), ("AB3", "#6.0854166666666671")]];
        let wb = read(&workbook(&[("Collecterooster", rows)])).unwrap();
        let sheet = wb.sheet("collecterooster").unwrap();
        assert_eq!(sheet.find("datum"), Some((1, 1)));
        assert_eq!(sheet.find_in_row(1, "Omschrijving"), Some(2));
        assert_eq!(sheet.text(3, 2), "Heilig Avondmaal");
        assert_eq!(sheet.text(3, 1), "46327");
        assert_eq!(sheet.cell(3, 28), Some(&Cell::Number(6.0854166666666671)));
        assert_eq!(sheet.last_row(), 3);
        assert_eq!(wb.date(46327.0).unwrap().to_string(), "2026-11-01");
        assert_eq!(wb.date(46023.0).unwrap().to_string(), "2026-01-01");
        assert!(read(b"geen zip").is_err());
    }

    #[test]
    fn cell_references() {
        assert_eq!(cell_ref("A1"), Some((1, 1)));
        assert_eq!(cell_ref("Q17"), Some((17, 17)));
        assert_eq!(cell_ref("AB3"), Some((3, 28)));
        assert_eq!(cell_ref("12"), None);
    }
}
