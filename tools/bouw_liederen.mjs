// De liederen die niet in het psalmboek staan (assets/data/liederen.json): voor "lied" en "na de
// dienst" met een titel, en de liederen van de bijzondere dagen uit de Excel van de kerk
// (tab Input, J2:K8).
//
// Gebruik:  CACHE=/pad/naar/cache node tools/bouw_liederen.mjs assets/data/liederen.json [tools/controle-liederen.txt]
//
// Een lied krijgt alleen een ingebouwde tekst als die zeker vrij is (dichter én vertaler vóór
// 1956 overleden) en woord voor woord gelijk in twee bronnen staat. Anders alleen titel en andere
// namen; de bediening zegt dan "tekst nog invullen" en de koster vult de tekst zelf in (in
// <data>/liederen.json, via Instellingen → Liederen).
//
//   Wilhelmus            tekst: anoniem, 1568-1572 (vrij). Bronnen: koninklijkhuis.nl ("Tekst van het
//                        Wilhelmus", hedendaagse spelling), nl.wikipedia.org en nl.wikisource.org
//                        ("Tegenwoordige tekst"). De tekst van koninklijkhuis.nl, per couplet alleen als
//                        een van de andere twee dezelfde woorden heeft; anders blijft dat couplet leeg.
//   Een vaste burcht is onze God   Luther (†1546); de Nederlandse vertaling die gezongen wordt, is
//                        niet zeker toe te schrijven: geen tekst.
//   Lam Gods             Decius (†1541); vertaler onzeker: geen tekst.
//   Kroon Hem met gouden kroon     Bridges (†1894) / Thring; Nederlandse vertaling (20e eeuw) niet vrij: geen tekst.
//   U zij de glorie      Budry (†1932); Nederlandse vertaling niet zeker vrij: geen tekst.
//   Heer, ik hoor van rijke zegen  Codner (†1919); vertaler onzeker: geen tekst.
//   Ere zij God          herkomst onzeker (Duits of Nederlands, c. 1870; dichter niet zeker): geen tekst.

import fs from "node:fs";
import path from "node:path";

const CACHE = process.env.CACHE || "liederen-cache";

async function get(url, file) {
  const p = path.join(CACHE, file);
  if (fs.existsSync(p)) return fs.readFileSync(p, "utf8");
  await new Promise((r) => setTimeout(r, 3000));
  const res = await fetch(url, { headers: { "User-Agent": "Mozilla/5.0 (streamandbeam; eenmalige import)" } });
  if (!res.ok) throw new Error(`${url}: HTTP ${res.status}`);
  const page = await res.text();
  fs.mkdirSync(path.dirname(p), { recursive: true });
  fs.writeFileSync(p, page);
  return page;
}

const ENTITIES = { amp: "&", lt: "<", gt: ">", quot: '"', apos: "'", nbsp: " " };
const text = (s) =>
  s
    .replace(/<[^>]+>/g, "")
    .replace(/&#x([0-9a-f]+);/gi, (_, n) => String.fromCodePoint(parseInt(n, 16)))
    .replace(/&#(\d+);/g, (_, n) => String.fromCodePoint(+n))
    .replace(/&([a-z]+);/gi, (m, n) => ENTITIES[n.toLowerCase()] ?? m)
    .replace(/[’‘]/g, "'")
    .replace(/\s+/g, " ")
    .trim();

// koninklijkhuis.nl: the present-day text is one <p>, an empty line (<br/><br/>) between the
// couplets; the original spelling follows under its own heading and is not used.
async function wilhelmusKoninklijkHuis() {
  const page = await get("https://www.koninklijkhuis.nl/onderwerpen/koninklijk-huis/volkslied/tekst-van-het-wilhelmus", "kh-wilhelmus.html");
  const p = [...page.matchAll(/<p[^>]*>([\s\S]*?)<\/p>/g)].map((m) => m[1]).find((b) => /Wilhelmus van Nassouwe<br\s*\/?>\s*ben ik, van Duitsen bloed/.test(b));
  if (!p) return [];
  return p.split(/<br\s*\/?>\s*<br\s*\/?>/i).map((c) => c.split(/<br\s*\/?>/i).map(text).filter(Boolean)).filter((c) => c.length);
}

// nl.wikipedia.org: the first <poem> ("Tegenwoordige tekst"), couplets under '''N-de couplet'''.
async function wilhelmusWikipedia() {
  const raw = await get("https://nl.wikipedia.org/w/index.php?title=Wilhelmus&action=raw", "wiki-wilhelmus.txt");
  const poem = raw.split("<poem>")[1].split("</poem>")[0];
  const unlink = (l) =>
    l.replace(/\[\[(?:[^|\]]*\|)?([^\]]*)\]\]/g, "$1").replace(/\{\{[^}]*\}\}/g, "").replace(/<ref[\s\S]*?(<\/ref>|\/>)/g, "").replace(/'''?/g, "");
  return poem
    .split(/'''[^']*couplet'''/i)
    .map((c) => c.split("\n").map((l) => text(unlink(l))).filter(Boolean))
    .filter((c) => c.length);
}

// nl.wikisource.org ("Het Wilhelmus"): a table, the present-day text in the first column.
async function wilhelmusWikisource() {
  const raw = await get("https://nl.wikisource.org/w/index.php?title=Het_Wilhelmus&action=raw", "ws-wilhelmus.txt");
  const col = raw.split("Originele tekst:")[1].split(/\n\|\n/)[1];
  return col
    .split(/'''[^']*couplet'''/i)
    .map((c) => c.split(/<br\s*\/?>/i).map(text).filter(Boolean))
    .filter((c) => c.length);
}

const words = (lines) =>
  lines.join(" ").toLowerCase().normalize("NFD").replace(/[̀-ͯ]/g, "").replace(/'/g, "").match(/[a-z]+/g) || [];

const [outFile, reportFile] = process.argv.slice(2);
if (!outFile) {
  console.error("gebruik: node tools/bouw_liederen.mjs assets/data/liederen.json [tools/controle-liederen.txt]");
  process.exit(2);
}

const kh = await wilhelmusKoninklijkHuis(), wp = await wilhelmusWikipedia(), ws = await wilhelmusWikisource();
const report = [
  "Controle liederen; gemaakt door tools/bouw_liederen.mjs",
  "",
  `Wilhelmus: koninklijkhuis.nl ${kh.length} coupletten, nl.wikipedia.org ${wp.length}, nl.wikisource.org ${ws.length}`,
  "  Een couplet krijgt de tekst van koninklijkhuis.nl als nl.wikipedia.org of nl.wikisource.org woord voor woord",
  "  hetzelfde heeft (leestekens tellen niet); anders blijft het leeg (tekst nog invullen).",
];
const stop = kh.length !== 15 || wp.length !== 15 || ws.length !== 15;
const wilhelmus = kh.map((c, i) => {
  const a = words(c).join(" ");
  const same = [["nl.wikipedia.org", wp[i]], ["nl.wikisource.org", ws[i]]].filter(([, o]) => words(o || []).join(" ") === a).map(([n]) => n);
  if (same.length) {
    report.push(`  couplet ${i + 1}: gelijk in ${same.join(" en ")}`);
    return c;
  }
  report.push(`  couplet ${i + 1}: LEEG, woorden verschillen
    koninklijkhuis.nl:  ${c.join(" / ")}
    nl.wikipedia.org:   ${(wp[i] || []).join(" / ")}
    nl.wikisource.org:  ${(ws[i] || []).join(" / ")}`);
  return [];
});
if (reportFile) fs.writeFileSync(reportFile, report.join("\n") + "\n");
else console.log(report.join("\n"));
if (stop) {
  console.error("Niet geschreven: een bron heeft niet 15 coupletten.");
  process.exit(1);
}

const data = {
  liederen: [
    { titel: "Wilhelmus", aliassen: ["Wilhelmus van Nassouwe", "Het Wilhelmus", "Volkslied"], bron: "tekst anoniem (1568-1572); koninklijkhuis.nl, per couplet gelijk aan nl.wikipedia.org of nl.wikisource.org (tools/controle-liederen.txt)", verzen: wilhelmus },
    { titel: "Een vaste burcht is onze God", aliassen: ["Een vaste burg is onze God", "Ein feste Burg"], bron: "", verzen: [] },
    { titel: "Lam Gods", aliassen: ["O Lam Gods onschuldig", "Lam Gods, onschuldig"], bron: "", verzen: [] },
    { titel: "Kroon Hem met gouden kroon", aliassen: [], bron: "", verzen: [] },
    { titel: "U zij de glorie", aliassen: ["U zij de glorie, opgestane Heer"], bron: "", verzen: [] },
    { titel: "Heer, ik hoor van rijke zegen", aliassen: [], bron: "", verzen: [] },
    { titel: "Ere zij God", aliassen: ["Ere zij God in de hoge"], bron: "", verzen: [] },
  ],
  // Excel, tab Input J2:K8 ("Speciale dagen"), zoals daar geschreven.
  feestdagen: [
    { dag: "Goede vrijdag", lied: "Lam Gods" },
    { dag: "Hemelvaart", lied: "Kroon Hem met gouden kroon" },
    { dag: "Pasen", lied: "U zij de Glorie" },
    { dag: "Pinksteren", lied: "Heer ik hoor van rijke zegen" },
    { dag: "Kerst", lied: "Ere zij God" },
    { dag: "Koningsdag", lied: "Wilhelmus" },
  ],
};
fs.writeFileSync(outFile, JSON.stringify(data, null, 1) + "\n");
console.log(`${outFile} geschreven`);
