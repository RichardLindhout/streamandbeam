//! The AI double check of the website: Claude reads the same page, and every difference
//! with the program's own reading becomes a warning in the service's "nakijken" list.
//! The program's reading stays; only a line it did not understand takes Claude's reading,
//! marked as read by AI.

use crate::books::{BibleRef, psalm_name};
use crate::claude::RawItem;
use crate::extract::{Extracted, ExtractedService};
use crate::parse::{and_list, from_ai};

/// Compares Claude's reading with the program's and adds the differences to the program's
/// services. Returns the number of differences and notes about the page as a whole.
pub fn double_check(own: &mut Extracted, ai: &Extracted) -> (usize, Vec<String>) {
    let mut count = 0;
    let mut notes = Vec::new();
    for svc in &mut own.diensten {
        match ai.diensten.iter().find(|a| same_service(svc, a)) {
            Some(a) => count += check_service(svc, a),
            None => {
                svc.niet_herkend.push("AI vond deze dienst niet op de website".into());
                count += 1;
            }
        }
    }
    for a in &ai.diensten {
        if !own.diensten.iter().any(|s| same_service(s, a)) {
            notes.push(format!("AI vond nog een dienst: {} {} {} (niet overgenomen)", a.datum, time(&a.tijd), a.dienst));
            count += 1;
        }
    }
    (count, notes)
}

/// "9:30" and "09:30" are the same time.
fn time(t: &str) -> String {
    match t.split_once([':', '.']) {
        Some((h, m)) => format!("{:02}:{m}", h.trim().parse::<u32>().unwrap_or(0)),
        None => t.to_string(),
    }
}

fn same_service(a: &ExtractedService, b: &ExtractedService) -> bool {
    a.datum == b.datum && time(&a.tijd) == time(&b.tijd)
}

/// What an item is about, without the details: the same psalm, the same chapter.
fn identity(r: &RawItem) -> String {
    match r.soort.as_str() {
        "psalm" | "slotzang" | "nazang" if r.psalm > 0 => format!("zang {}", r.psalm),
        "lezing" => format!("lezing {} {}", r.boek, r.hoofdstuk),
        "catechismus" => "catechismus".into(),
        "lied" | "nazang" => "lied".into(),
        "belijdenis" | "formulier" => format!("{} {}", r.soort, r.document),
        other => format!("{other} {}", simple(&r.opschrift)),
    }
}

fn simple(s: &str) -> String {
    crate::parse::fold(s).chars().filter(|c| c.is_alphanumeric()).collect()
}

/// The details that should be the same.
fn details(r: &RawItem) -> String {
    match r.soort.as_str() {
        "psalm" | "slotzang" | "nazang" if r.psalm > 0 => format!("{:?}", r.verzen),
        "lezing" => {
            // "13:1-23" is the whole of a chapter of 23 verses, like "13" (1-999).
            let last = crate::parse::last_verse(r.boek, r.hoofdstuk);
            let to = r.tot.max(r.van).min(last);
            if r.van <= 1 && to >= last { "heel".into() } else { format!("{}-{to}", r.van) }
        }
        "catechismus" => {
            let mut q = r.vragen.clone();
            q.sort();
            if q.is_empty() { format!("zondag {}", r.zondag) } else { format!("{q:?}") }
        }
        "lied" | "nazang" => simple(&r.opschrift),
        "belijdenis" => format!("{} {:?}", r.hoofdstuk, r.artikelen),
        _ => String::new(),
    }
}

fn verses(v: &[u32]) -> String {
    and_list(&v.iter().map(u32::to_string).collect::<Vec<_>>())
}

/// In words: "Psalm 27 vers 3, 4, 5, 6 en 7", "Habakuk 3:17-19", "Catechismus vraag 58".
pub fn describe(r: &RawItem) -> String {
    match r.soort.as_str() {
        "psalm" | "slotzang" | "nazang" if r.psalm > 0 => format!("{} vers {}", psalm_name(r.psalm), verses(&r.verzen)),
        "lezing" => BibleRef { book: r.boek, chapter: r.hoofdstuk, from: r.van.max(1), to: r.tot.max(r.van) }.to_string(),
        "catechismus" if r.vragen.is_empty() => format!("Catechismus zondag {}", r.zondag),
        "catechismus" => format!("Catechismus vraag {}", verses(&r.vragen)),
        "lied" | "nazang" => format!("lied '{}'", r.opschrift),
        "belijdenis" => {
            let short = crate::confessions::short(&r.document, r.hoofdstuk, &r.artikelen);
            if short.is_empty() { r.document.clone() } else { short }
        }
        "formulier" => crate::confessions::form(&r.document).map_or_else(|| r.document.clone(), |f| f.kort.clone()),
        "" => "niets".into(),
        _ => r.opschrift.clone(),
    }
}

/// The program's reading next to Claude's, short when only the verses differ: "vers 3 en 7".
fn describe_beside(own: &RawItem, ai: &RawItem) -> String {
    let sung = matches!(own.soort.as_str(), "psalm" | "slotzang" | "nazang") && own.psalm > 0 && own.psalm == ai.psalm;
    if sung { format!("vers {}", verses(&own.verzen)) } else { describe(own) }
}

/// Lines up two lists of items (the longest common order), pairing items about the same
/// thing, and a line the program did not understand with whatever Claude read there.
fn align(own: &[RawItem], ai: &[RawItem]) -> Vec<(Option<usize>, Option<usize>)> {
    let score = |a: &RawItem, b: &RawItem| -> u32 {
        if identity(a) == identity(b) {
            if details(a) == details(b) { 3 } else { 2 }
        } else if a.niet_begrepen {
            1
        } else {
            0
        }
    };
    let (n, m) = (own.len(), ai.len());
    let mut best = vec![vec![0u32; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            let s = score(&own[i], &ai[j]);
            let pair = if s > 0 { s + best[i + 1][j + 1] } else { 0 };
            best[i][j] = pair.max(best[i + 1][j]).max(best[i][j + 1]);
        }
    }
    let mut out = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < n || j < m {
        if i < n && j < m {
            let s = score(&own[i], &ai[j]);
            if s > 0 && best[i][j] == s + best[i + 1][j + 1] {
                out.push((Some(i), Some(j)));
                i += 1;
                j += 1;
                continue;
            }
        }
        if i < n && (j == m || best[i][j] == best[i + 1][j]) {
            out.push((Some(i), None));
            i += 1;
        } else {
            out.push((None, Some(j)));
            j += 1;
        }
    }
    out
}

/// The text a line was read from, or the program's reading when it has none.
fn source(r: &RawItem) -> String {
    if r.bron.is_empty() { describe(r) } else { r.bron.clone() }
}

/// Takes Claude's reading for a line the program did not understand, and replaces the
/// line's earlier "not understood" in the list.
fn take_from_ai(svc: &mut ExtractedService, at: usize, ai: &RawItem) {
    let bron = svc.onderdelen[at].bron.clone();
    svc.niet_herkend.retain(|p| !p.starts_with(&format!("{bron}:")));
    svc.niet_herkend.push(format!("'{bron}' niet herkend door het programma; AI leest {}: nakijken", describe(ai)));
    svc.onderdelen[at] = from_ai(ai.clone(), &bron);
}

fn check_service(svc: &mut ExtractedService, ai: &ExtractedService) -> usize {
    let mut warnings: Vec<String> = Vec::new();
    let mut take: Vec<(usize, RawItem)> = Vec::new();
    for pair in align(&svc.onderdelen, &ai.onderdelen) {
        match pair {
            (Some(i), Some(j)) => {
                let (own, theirs) = (&svc.onderdelen[i], &ai.onderdelen[j]);
                if own.niet_begrepen {
                    take.push((i, theirs.clone()));
                } else if details(own) != details(theirs) {
                    warnings.push(format!("AI leest '{}' anders: {} (programma: {})", source(own), describe(theirs), describe_beside(own, theirs)));
                }
            }
            (Some(i), None) => {
                let own = &svc.onderdelen[i];
                if !own.niet_begrepen {
                    warnings.push(format!("AI ziet '{}' niet (programma: {})", source(own), describe(own)));
                }
            }
            (None, Some(j)) => warnings.push(format!("AI leest nog {}; het programma niet: nakijken", describe(&ai.onderdelen[j]))),
            (None, None) => {}
        }
    }
    let mut count = warnings.len() + take.len();
    for (i, theirs) in take {
        take_from_ai(svc, i, &theirs);
    }
    // The sermon text.
    let (own, theirs) = (&svc.preektekst, &ai.preektekst);
    if own.niet_begrepen && !theirs.soort.is_empty() {
        let bron = own.bron.clone();
        svc.niet_herkend.retain(|p| !p.starts_with(&format!("{bron}:")));
        svc.niet_herkend.push(format!("Tekst '{bron}' niet herkend door het programma; AI leest {}: nakijken", describe(theirs)));
        svc.preektekst = from_ai(theirs.clone(), &bron);
        count += 1;
    } else if !(own.soort.is_empty() && theirs.soort.is_empty()) && (identity(own) != identity(theirs) || details(own) != details(theirs)) {
        let own = if own.soort.is_empty() { "geen tekst".to_string() } else { describe(own) };
        let theirs = if theirs.soort.is_empty() { "geen tekst".to_string() } else { describe(theirs) };
        warnings.push(format!("AI leest de tekst anders: {theirs} (programma: {own})"));
        count += 1;
    }
    if simple(&svc.thema) != simple(&ai.thema) {
        let own = if svc.thema.is_empty() { "geen thema".to_string() } else { format!("'{}'", svc.thema) };
        let theirs = if ai.thema.is_empty() { "geen thema".to_string() } else { format!("'{}'", ai.thema) };
        warnings.push(format!("AI leest het thema anders: {theirs} (programma: {own})"));
        count += 1;
    }
    let points = |p: &[String]| p.iter().map(|s| simple(s)).collect::<Vec<_>>();
    if points(&svc.punten) != points(&ai.punten) {
        let list = |p: &[String]| if p.is_empty() { "geen".to_string() } else { p.iter().enumerate().map(|(i, s)| format!("{}. {s}", i + 1)).collect::<Vec<_>>().join(" / ") };
        warnings.push(format!("AI leest de punten anders: {} (programma: {})", list(&ai.punten), list(&svc.punten)));
        count += 1;
    }
    svc.niet_herkend.extend(warnings);
    count
}

#[cfg(test)]
mod tests {
    use super::*;

    fn psalm(n: u32, verses: &[u32], bron: &str) -> RawItem {
        RawItem { soort: "psalm".into(), opschrift: "Zingen".into(), psalm: n, verzen: verses.to_vec(), herkend: true, bron: bron.into(), ..Default::default() }
    }

    fn reading(book: u32, chapter: u32, from: u32, to: u32) -> RawItem {
        RawItem { soort: "lezing".into(), opschrift: "Schriftlezing".into(), boek: book, hoofdstuk: chapter, van: from, tot: to, herkend: true, ..Default::default() }
    }

    fn service(items: Vec<RawItem>) -> ExtractedService {
        ExtractedService {
            datum: "2026-11-01".into(),
            tijd: "16:00".into(),
            dienst: "Middagdienst".into(),
            onderdelen: items,
            thema: "Verrast door vreugde".into(),
            punten: vec!["Nu al".into(), "Straks volkomen".into()],
            ..Default::default()
        }
    }

    fn pages(own: ExtractedService, ai: ExtractedService) -> (Extracted, usize, Vec<String>) {
        let mut own = Extracted { diensten: vec![own], ..Default::default() };
        let ai = Extracted { diensten: vec![ai], ..Default::default() };
        let (n, notes) = double_check(&mut own, &ai);
        (own, n, notes)
    }

    #[test]
    fn same_reading_no_warnings() {
        let s = service(vec![psalm(90, &[1], "Ps. 90: 1"), reading(35, 3, 17, 19), reading(9, 13, 1, 999)]);
        let mut ai = s.clone();
        ai.tijd = "16:00".into();
        ai.onderdelen[0].opschrift = "Voorzang".into();
        // The whole chapter, written as its verses (1 Samuël 13 has 23).
        ai.onderdelen[2].tot = 23;
        let (own, n, notes) = pages(s, ai);
        assert_eq!(n, 0, "{:?}", own.diensten[0].niet_herkend);
        assert!(notes.is_empty() && own.diensten[0].niet_herkend.is_empty());
    }

    #[test]
    fn a_different_reading_is_a_warning_and_the_program_stays() {
        let s = service(vec![psalm(90, &[1], "Ps. 90: 1"), psalm(27, &[3, 7], "Ps. 27: 3-7"), psalm(43, &[4], "Ps. 43: 4")]);
        let ai = service(vec![psalm(90, &[1], ""), psalm(27, &[3, 4, 5, 6, 7], ""), psalm(43, &[4], ""), reading(66, 7, 9, 12)]);
        let (own, n, _) = pages(s, ai);
        let svc = &own.diensten[0];
        assert_eq!(n, 2);
        assert_eq!(svc.niet_herkend[0], "AI leest 'Ps. 27: 3-7' anders: Psalm 27 vers 3, 4, 5, 6 en 7 (programma: vers 3 en 7)");
        assert_eq!(svc.niet_herkend[1], "AI leest nog Openbaring 7:9-12; het programma niet: nakijken");
        assert_eq!(svc.onderdelen[1].verzen, vec![3, 7]);
        assert_eq!(svc.onderdelen.len(), 3);
    }

    #[test]
    fn missing_items_theme_and_points() {
        let s = service(vec![psalm(90, &[1], "Ps. 90: 1"), psalm(147, &[1], "Ps. 147: 1")]);
        let mut ai = service(vec![psalm(90, &[1], "")]);
        ai.thema = "Verrast door de vreugde".into();
        ai.punten = vec!["Nu al".into()];
        let (own, n, _) = pages(s, ai);
        let p = &own.diensten[0].niet_herkend;
        assert_eq!(n, 3, "{p:?}");
        assert_eq!(p[0], "AI ziet 'Ps. 147: 1' niet (programma: Psalm 147 vers 1)");
        assert!(p[1].starts_with("AI leest het thema anders: 'Verrast door de vreugde'"));
        assert!(p[2].starts_with("AI leest de punten anders: 1. Nu al (programma: 1. Nu al / 2. Straks volkomen)"));
    }

    #[test]
    fn a_line_the_program_did_not_understand_takes_the_ai_reading() {
        let unknown = RawItem {
            soort: "mededeling".into(),
            opschrift: "Ps. 18: Vz".into(),
            bron: "Ps. 18: Vz".into(),
            niet_begrepen: true,
            reden: "niet herkend".into(),
            ..Default::default()
        };
        let mut s = service(vec![psalm(90, &[1], "Ps. 90: 1"), unknown]);
        s.niet_herkend.push("Ps. 18: Vz: niet herkend".into());
        let ai = service(vec![psalm(90, &[1], ""), psalm(18, &[1], "")]);
        let (own, n, _) = pages(s, ai);
        let svc = &own.diensten[0];
        assert_eq!(n, 1);
        assert_eq!((svc.onderdelen[1].soort.as_str(), svc.onderdelen[1].psalm), ("psalm", 18));
        assert!(!svc.onderdelen[1].herkend && svc.onderdelen[1].reden.contains("door AI gelezen"));
        assert_eq!(svc.niet_herkend, ["'Ps. 18: Vz' niet herkend door het programma; AI leest Psalm 18 vers 1: nakijken"]);
    }

    #[test]
    fn services_only_one_side_found() {
        let mut own = Extracted { diensten: vec![service(vec![])], ..Default::default() };
        let mut other = service(vec![]);
        other.tijd = "9:30".into();
        other.dienst = "Morgendienst".into();
        let ai = Extracted { diensten: vec![other], ..Default::default() };
        let (n, notes) = double_check(&mut own, &ai);
        assert_eq!(n, 2);
        assert_eq!(own.diensten[0].niet_herkend, ["AI vond deze dienst niet op de website"]);
        assert_eq!(notes, ["AI vond nog een dienst: 2026-11-01 09:30 Morgendienst (niet overgenomen)"]);
    }

    #[test]
    fn the_sermon_text() {
        let mut s = service(vec![]);
        s.preektekst = RawItem { soort: "catechismus".into(), vragen: vec![58], herkend: true, ..Default::default() };
        let mut ai = s.clone();
        ai.preektekst.vragen = vec![57];
        let (own, n, _) = pages(s, ai);
        assert_eq!(n, 1);
        assert_eq!(own.diensten[0].niet_herkend, ["AI leest de tekst anders: Catechismus vraag 57 (programma: Catechismus vraag 58)"]);
    }
}
