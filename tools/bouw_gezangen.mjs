// De "Enige Gezangen" uit het psalmboek (berijming 1773), in de uitgave van de Gereformeerde
// Bijbelstichting (GBS), zoals in de GBS-psalmboeken van de HHK. Bron: GBS Bijbel Online
// (https://bijbel-statenvertaling.com/1773/gezang/<naam>/), de site van de GBS zelf.
//
// Gebruik:  CACHE=/pad/naar/cache node tools/bouw_gezangen.mjs assets/data/psalmen.json [tools/controle-gezangen.txt]
//
// Nummering: 150 + het nummer van het gezang in de Excel van de kerk (tab Input), dat ook
// online-bijbel.nl en liturgie.nu gebruiken: 151 De Tien Geboden des HEEREN ... 156 en 157 de
// eerste en de tweede berijming van De Twaalf Artikelen des Geloofs ... 162 Avondzang.
// Psalm 1-150 in psalmen.json blijven zoals ze zijn; 151-162 worden (opnieuw) geschreven.
//
// Wat er met de GBS-tekst gebeurt (en verder niets), net als bij de psalmen:
//   - typografie: ’ ‘ -> ' en “ ” -> ", spaties enkel;
//   - melodiestreepjes eruit ("welda - - dig" -> "weldadig");
//   - besluiten uit CORRECTIES hieronder (nu geen).
//
// Controles, tegen liturgie.nu en online-bijbel.nl:
//   - aantal verzen per gezang en regels per vers gelijk;
//   - geen lege verzen of regels, geen HTML-resten;
//   - woord voor woord: elk woord van de GBS moet op dezelfde plaats in minstens één andere
//     uitgave staan; elk verschil met elke bron staat in het verslag. Een GBS-woord dat geen
//     andere bron heeft, stopt het script tenzij er een besluit in CORRECTIES staat.

import fs from "node:fs";
import path from "node:path";

const CACHE = process.env.CACHE || "gezangen-cache";

// Het nummer in de lijst van de kerk, de naam in het programma, de pagina's in de drie bronnen.
const GEZANGEN = [
  [1, "De Tien Geboden des HEEREN", "de-tien-geboden-des-heeren", "tien-geboden"],
  [2, "De Lofzang van Maria", "de-lofzang-van-maria", "lofzang-van-maria"],
  [3, "De Lofzang van Zacharias", "de-lofzang-van-zacharias", "lofzang-van-zacharias"],
  [4, "De Lofzang van Simeon", "de-lofzang-van-simeon", "lofzang-van-simeon"],
  [5, "Het Gebed des HEEREN", "het-gebed-des-heeren", "gebed-des-heeren"],
  [6, "De Twaalf Artikelen des Geloofs (eerste berijming)", "de-eerste-berijming-van-de-twaalf-artikelen-des-geloofs", "twaalf-artikelen-berijming-1"],
  [7, "De Twaalf Artikelen des Geloofs (tweede berijming)", "de-tweede-berijming-van-de-twaalf-artikelen-des-geloofs", "twaalf-artikelen-berijming-2"],
  [8, "Bedezang voor de predikatie", "bedezang-voor-de-predicatie", "bedezang-voor-de-predikatie"],
  [9, "Morgenzang", "morgenzang", "morgenzang"],
  [10, "Bedezang voor het eten", "bedezang-voor-het-eten", "bedezang-voor-het-eten"],
  [11, "Dankzang na het eten", "dankzang-na-het-eten", "dankzang-na-het-eten"],
  [12, "Avondzang", "avondzang", "avondzang"],
];

// Besluiten over GBS-woorden die geen andere bron heeft: { "151:3": { "fout": "goed", "=": "reden" } }.
// Alleen "=" is: nagekeken, de GBS-tekst blijft.
const CORRECTIES = {
  "155:6": { "=": "'wie ons misdeên' (liturgie.nu en online-bijbel.nl: 'die ons misdeên', zoals 1773): eigen lezing van de GBS, de gemeente zingt uit het GBS-boek; gehouden" },
  "156:2": { "=": "'heilsgemeenschap' (anderen: 'heilgemeenschap', zoals 1773): hedendaagse spelling van de GBS; gehouden" },
};

async function get(url, file) {
  const p = path.join(CACHE, file);
  if (fs.existsSync(p)) return fs.readFileSync(p, "utf8");
  for (let attempt = 0; attempt < 5; attempt++) {
    try {
      await new Promise((r) => setTimeout(r, 3000));
      const res = await fetch(url, { headers: { "User-Agent": "Mozilla/5.0 (streamandbeam; eenmalige import)" } });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      const page = await res.text();
      fs.mkdirSync(path.dirname(p), { recursive: true });
      fs.writeFileSync(p, page);
      return page;
    } catch (e) {
      console.log(`  wacht op ${url}: ${e.message}`);
      await new Promise((r) => setTimeout(r, 30000));
    }
  }
  throw new Error(`kan ${url} niet ophalen`);
}

const ENTITIES = { amp: "&", lt: "<", gt: ">", quot: '"', apos: "'", nbsp: " ", rsquo: "’", lsquo: "‘", rdquo: "”", ldquo: "“", hellip: "…" };

function text(s) {
  s = s.replace(/<[^>]+>/g, "");
  s = s.replace(/&#(\d+);/g, (_, n) => String.fromCodePoint(+n)).replace(/&#x([0-9a-f]+);/gi, (_, n) => String.fromCodePoint(parseInt(n, 16)));
  s = s.replace(/&([a-z]+);/gi, (m, n) => ENTITIES[n.toLowerCase()] ?? m).replace(/ /g, " ");
  return s.replace(/\s+/g, " ").trim();
}

const lines = (body) => body.split(/<br\s*\/?>/i).map(text).filter(Boolean);

async function gbs(slug) {
  let page = await get(`https://bijbel-statenvertaling.com/1773/gezang/${slug}/`, `gbs/${slug}.html`);
  page = page.split("psalm-content publication-content")[1];
  const out = {};
  for (const m of page.matchAll(/<div class="verse verse-(\d+) psalm psalm-\d+[^"]*" id="\d+">[\s\S]*?<span class="verse-text">([\s\S]*?)<\/span>\s*<\/div>/g)) {
    out[+m[1]] = lines(m[2]);
  }
  return out;
}

async function liturgie(slug) {
  let page = await get(`https://www.liturgie.nu/gezangen/${slug}`, `lit/${slug}.html`);
  page = page.split("psalm_tekst")[1].split(/class="tab content/)[0];
  page = page.replace(/<i class="begrip"[^>]*?data-tippy-content="[\s\S]*?" data-begrip-real-id="\d+">/g, "");
  const out = {};
  for (const m of page.matchAll(/id="vers-(\d+)"[\s\S]*?<p class="verstekst">([\s\S]*?)<\/p>/g)) out[+m[1]] = lines(m[2]);
  return out;
}

async function onlineBijbel(nr) {
  const page = await get(`https://www.online-bijbel.nl/12gezang/${nr}/`, `ob/${nr}.html`);
  const out = {};
  for (const m of page.matchAll(/data-vers='(\d+)'[\s\S]*?<\/span>\s*([\s\S]*?)<\/p>/g)) {
    const l = lines(m[2]);
    if (l.length) l[0] = l[0].replace(/^\d+\s+/, "");
    if (l.length && !out[+m[1]]) out[+m[1]] = l;
  }
  return out;
}

function clean(line) {
  line = line.replace(/[’‘]/g, "'").replace(/[“”]/g, '"');
  line = line.replace(/(\w) -(?: -)* (\w)/g, "$1$2");
  return line.replace(/\s+/g, " ").trim();
}

function tokens(ls) {
  let s = ls.join(" ").toLowerCase().replace(/’/g, "'").normalize("NFD").replace(/[̀-ͯ]/g, "").replace(/'/g, "");
  s = s.replace(/(\w)-\s*(\w)/g, "$1$2");
  return s.match(/[a-z]+/g) || [];
}

// The word-by-word differences between a and b (LCS), as ["-word", "+word"] runs.
function diff(a, b) {
  const n = a.length, m = b.length;
  const dp = Array.from({ length: n + 1 }, () => new Int32Array(m + 1));
  for (let i = n - 1; i >= 0; i--) for (let j = m - 1; j >= 0; j--) dp[i][j] = a[i] === b[j] ? dp[i + 1][j + 1] + 1 : Math.max(dp[i + 1][j], dp[i][j + 1]);
  const same = new Array(n).fill(false), out = [];
  let i = 0, j = 0, del = [], add = [];
  const flush = () => {
    if (del.length || add.length) out.push(`${del.join(" ") || "∅"} -> ${add.join(" ") || "∅"}`);
    del = []; add = [];
  };
  while (i < n || j < m) {
    if (i < n && j < m && a[i] === b[j]) { flush(); same[i] = true; i++; j++; }
    else if (j < m && (i === n || dp[i][j + 1] >= dp[i + 1][j])) add.push(b[j++]);
    else del.push(a[i++]);
  }
  flush();
  return { same, changes: out };
}

const [outFile, reportFile] = process.argv.slice(2);
if (!outFile) {
  console.error("gebruik: node tools/bouw_gezangen.mjs assets/data/psalmen.json [tools/controle-gezangen.txt]");
  process.exit(2);
}

const report = ["Controle Enige Gezangen (berijming 1773, GBS-uitgave); gemaakt door tools/bouw_gezangen.mjs", ""];
const result = {};
let stop = false, verses = 0;
for (const [nr, naam, gSlug, lSlug] of GEZANGEN) {
  const g = await gbs(gSlug), l = await liturgie(lSlug), o = await onlineBijbel(nr);
  const n = Object.keys(g).length;
  report.push(`${150 + nr} = gezang ${nr}, ${naam}`);
  report.push(`  verzen: GBS ${n}  liturgie.nu ${Object.keys(l).length}  online-bijbel.nl ${Object.keys(o).length}`);
  if (!n || n !== Object.keys(l).length || n !== Object.keys(o).length) { report.push("  FOUT: aantal verzen verschilt"); stop = true; }
  const verzen = [];
  for (let v = 1; v <= n; v++) {
    const fix = CORRECTIES[`${150 + nr}:${v}`] || {};
    let gl = (g[v] || []).map(clean);
    for (const [wrong, right] of Object.entries(fix)) if (wrong !== "=") gl = gl.map((x) => x.replace(new RegExp(`(?<![\\w'])${wrong.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}(?![\\w'])`, "g"), right));
    if (!gl.length || gl.some((x) => !x || /[<>&]/.test(x))) { report.push(`  FOUT: vers ${v} leeg of met HTML`); stop = true; }
    const lit = (l[v] || []).map(clean), ob = (o[v] || []).map(clean);
    if (gl.length !== lit.length || gl.length !== ob.length) {
      report.push(`  FOUT: vers ${v}: regels GBS ${gl.length}, liturgie.nu ${lit.length}, online-bijbel.nl ${ob.length}`);
      stop = true;
    }
    const gt = tokens(gl);
    const dl = diff(gt, tokens(lit)), dob = diff(gt, tokens(ob));
    // A word the others write as two ("teweeggebracht" / "teweeg gebracht") is spelling, not
    // another word: it counts as found when the letters are the same.
    const spelling = (d) => d.changes.every((c) => { const [a, b] = c.split(" -> "); return a.replace(/ /g, "") === b.replace(/ /g, ""); });
    const lonely = gt.filter((_, i) => !(dl.same[i] || spelling(dl)) && !(dob.same[i] || spelling(dob)));
    for (const [src, d, other] of [["liturgie.nu     ", dl, lit], ["online-bijbel.nl", dob, ob]]) {
      for (const c of d.changes) report.push(`  vers ${v}: ${src} ${c}${spelling({ changes: [c] }) ? "  (alleen aaneen/los)" : ""}`);
      if (d.changes.length) {
        gl.forEach((line, i) => {
          if (tokens([line]).join(" ") !== tokens([other[i] || ""]).join(" ")) report.push(`      GBS: ${line}\n      ${src.trim()}: ${other[i] || ""}`);
        });
      }
    }
    if (lonely.length && !fix["="]) { report.push(`  FOUT: vers ${v}: alleen in de GBS: ${lonely.join(" ")} (besluit nodig)`); stop = true; }
    if (fix["="]) report.push(`  vers ${v}: besluit: ${fix["="]}`);
    verzen.push(gl);
  }
  verses += verzen.length;
  report.push("");
  result[String(150 + nr)] = { naam, verzen };
}
report.splice(1, 0, `${verses} verzen in ${GEZANGEN.length} gezangen; per vers de woorden die liturgie.nu of online-bijbel.nl anders heeft (GBS -> bron).`);

if (reportFile) fs.writeFileSync(reportFile, report.join("\n") + "\n");
else console.log(report.join("\n"));
if (stop) {
  console.error("Niet geschreven: zie de FOUT-regels in het verslag.");
  process.exit(1);
}
const all = JSON.parse(fs.readFileSync(outFile, "utf8"));
for (const k of Object.keys(all)) if (+k > 150) delete all[k];
Object.assign(all, result);
// Written as before (Python's json.dump: ", " and ": ", no escapes beyond JSON's own).
const py = (v) => (Array.isArray(v) ? "[" + v.map(py).join(", ") + "]" : v && typeof v === "object" ? "{" + Object.entries(v).map(([k, x]) => JSON.stringify(k) + ": " + py(x)).join(", ") + "}" : JSON.stringify(v));
fs.writeFileSync(outFile, py(all));
console.log(`${outFile}: gezangen 151-${150 + GEZANGEN.length} geschreven`);
