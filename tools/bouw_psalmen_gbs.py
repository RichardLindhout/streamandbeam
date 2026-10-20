"""Psalmen 1-150, berijming 1773 in de uitgave van de Gereformeerde Bijbelstichting (GBS), zoals in
de GBS-psalmboeken van de Hersteld Hervormde Kerk. Bron: GBS Bijbel Online
(https://bijbel-statenvertaling.com/1773/psalm/N/), de site van de GBS zelf.

Gebruik:  CACHE=/pad/naar/cache python3 tools/bouw_psalmen_gbs.py assets/data/psalmen.json [tools/controle-psalmen.txt]

Wat er met de GBS-tekst gebeurt (en verder niets):
  - typografie: ’ ‘ -> ' en “ ” -> " (zoals de rest van assets/data), spaties enkel;
  - melodiestreepjes eruit ("welda - - dig" -> "weldadig"; de GBS zet die bij lange noten);
  - besluiten uit tools/psalm-correcties.json: "9:10": {"vluchtten": "vluchten", "=": "reden"}
    vervangt woorden; een vers met alleen {"=": "reden"} is nagekeken en blijft zoals de GBS het heeft.

Controles, tegen liturgie.nu, online-bijbel.nl, psalmboek.nl (psalm 1-36, tools/psalmen-1-36-psalmboek.json)
en de uitgave van 1774 (DBNL):
  - aantal verzen per psalm en regels per vers gelijk aan liturgie.nu en online-bijbel.nl;
  - geen lege verzen of regels, geen HTML-resten;
  - elk woord van de GBS moet op dezelfde plaats in minstens één andere uitgave (liturgie.nu,
    online-bijbel.nl, psalmboek.nl) voorkomen. Zo niet, dan is het een tikfout van de GBS óf een
    eigen lezing van de GBS; dat vers moet dan een besluit in psalm-correcties.json hebben.
Zonder besluit stopt het script en wordt psalmen.json niet geschreven.
"""

import difflib
import html
import json
import os
import re
import sys
import time
import unicodedata
import urllib.request

CACHE = os.environ.get("CACHE", "/tmp/psalmen-cache")
UA = {"User-Agent": "Mozilla/5.0 (streamandbeam; eenmalige import)"}


def get(url, path):
    """Fetch once, politely (4 s between requests), and keep the page in the cache."""
    path = os.path.join(CACHE, path)
    if os.path.exists(path):
        return open(path, encoding="utf-8").read()
    for attempt in range(5):
        try:
            time.sleep(4)
            page = urllib.request.urlopen(urllib.request.Request(url, headers=UA), timeout=60).read().decode("utf-8", "replace")
            os.makedirs(os.path.dirname(path), exist_ok=True)
            open(path, "w", encoding="utf-8").write(page)
            return page
        except Exception as e:
            print(f"  wacht op {url}: {e}", flush=True)
            time.sleep(60)
    raise RuntimeError(f"kan {url} niet ophalen")


def text(s):
    s = re.sub(r"<[^>]+>", "", s)
    s = html.unescape(s).replace("\xa0", " ")
    return re.sub(r"\s+", " ", s).strip()


def gbs(n):
    page = get(f"https://bijbel-statenvertaling.com/1773/psalm/{n}/", f"gbs/{n}.html")
    page = page.split("psalm-content publication-content", 1)[1]
    out = {}
    for v, body in re.findall(r'(?s)<div class="verse verse-(\d+) psalm psalm-\d+[^"]*" id="\d+">.*?<span class="verse-text">(.*?)</span>\s*</div>', page):
        out[int(v)] = [x for x in (text(l) for l in re.split(r"<br\s*/?>", body)) if x]
    return out


def liturgie(n):
    page = get(f"https://www.liturgie.nu/psalmen/{n}", f"lit/{n}.html")
    page = re.split(r'class="tab content', page.split("psalm_tekst", 1)[1])[0]
    # Word explanations sit in attributes that may themselves contain <br>.
    page = re.sub(r'(?s)<i class="begrip"[^>]*?data-tippy-content=".*?" data-begrip-real-id="\d+">', "", page)
    out = {}
    for v, body in re.findall(r'(?s)id="vers-(\d+)".*?<p class="verstekst">(.*?)</p>', page):
        out[int(v)] = [x for x in (text(l) for l in re.split(r"<br\s*/?>", body)) if x]
    return out


def online_bijbel(n):
    page = get(f"https://www.online-bijbel.nl/psalm/{n}/", f"ob/{n}.html")
    out = {}
    for num, body in re.findall(r"(?s)data-vers='(\d+)'.*?</span>\s*(.*?)</p>", page):
        lines = [x for x in (text(l) for l in re.split(r"<br\s*/?>", body)) if x]
        if lines:
            lines[0] = re.sub(r"^\d+\s+", "", lines[0])
        out[int(num)] = lines
    return out


def dbnl(n):
    page = get(f"https://www.dbnl.org/tekst/_boe008boek01_01/_boe008boek01_01_{12 + n:04d}.php", f"dbnl/{n}.html")
    parts = re.split(r"<h4>&sect;\d+</h4>", page)[1:]
    if parts:
        parts[-1] = parts[-1].split('id="right"')[0]
    return {
        i: [text(l) for l in re.findall(r'<div class="line-content">(.*?)</div>', p) if "PAUZE" not in l]
        for i, p in enumerate(parts, 1)
    }


PSALMBOEK = json.load(open("tools/psalmen-1-36-psalmboek.json", encoding="utf-8"))


def psalmboek(n):
    p = PSALMBOEK.get(str(n))
    return {i: v for i, v in enumerate(p["verzen"], 1)} if p else {}


def clean(line):
    line = line.replace("’", "'").replace("‘", "'").replace("“", '"').replace("”", '"')
    line = re.sub(r"(\w) -(?: -)* (\w)", r"\1\2", line)  # melody separators
    return re.sub(r"\s+", " ", line).strip()


def tokens(lines):
    s = unicodedata.normalize("NFD", " ".join(lines).lower().replace("’", "'"))
    s = "".join(c for c in s if not unicodedata.combining(c)).replace("'", "")
    s = re.sub(r"(\w)-\s*(\w)", r"\1\2", s)
    return re.findall(r"[a-z]+", s)


def apply(lines, fixes):
    out = []
    for l in lines:
        for wrong, right in fixes.items():
            if wrong != "=":
                l = re.sub(rf"(?<![\w']){re.escape(wrong)}(?![\w'])", right, l)
        out.append(l)
    return out


def unsupported(lines, others):
    """GBS words that no other edition has at that place."""
    g = tokens(lines)
    seen = [False] * len(g)
    for other in others:
        if not other:
            continue
        for t, i1, i2, j1, j2 in difflib.SequenceMatcher(None, g, tokens(other), autojunk=False).get_opcodes():
            if t == "equal":
                for i in range(i1, i2):
                    seen[i] = True
    return [g[i] for i in range(len(g)) if not seen[i]]


def main(out_path, report_path="tools/controle-psalmen.txt"):
    corrections = json.load(open("tools/psalm-correcties.json", encoding="utf-8"))
    data, report, problems, counts = {}, [], [], []
    for n in range(1, 151):
        g, lit, ob, old, pb = gbs(n), liturgie(n), online_bijbel(n), dbnl(n), psalmboek(n)
        counts.append(f"  Psalm {n:3}: GBS {len(g):2}  liturgie.nu {len(lit):2}  online-bijbel.nl {len(ob):2}  1774 {len(old):2}" + (f"  psalmboek.nl {len(pb):2}" if pb else ""))
        if sorted(g) != list(range(1, len(g) + 1)) or len(g) != len(ob) or (len(g) != len(lit) and n != 18):
            problems.append(f"Psalm {n}: aantal verzen GBS {len(g)}, liturgie.nu {len(lit)}, online-bijbel.nl {len(ob)}")
        verses = []
        for v in range(1, len(g) + 1):
            key = f"{n}:{v}"
            fixes = corrections.get(key, {})
            lines = apply([clean(l) for l in g[v]], fixes)
            if not lines or any(not l for l in lines) or any(re.search(r"[<>&]|\d", l) for l in lines):
                problems.append(f"{key}: lege regel of HTML-rest: {lines}")
            if len(lines) != len(ob.get(v, [])) or (n != 18 and len(lines) != len(lit.get(v, []))):
                problems.append(f"{key}: {len(lines)} regels, liturgie.nu {len(lit.get(v, []))}, online-bijbel.nl {len(ob.get(v, []))}")
            alone = unsupported(lines, [lit.get(v), ob.get(v), pb.get(v)])
            if any(k != "=" for k in fixes):
                report.append(f"{key} VERBETERD: {fixes.get('=', '')}\n    GBS: {' / '.join(clean(l) for l in g[v])}\n    nu:  {' / '.join(lines)}")
            elif "=" in fixes:
                report.append(f"{key} nagekeken: {fixes['=']}")
            if alone and "=" not in fixes:
                problems.append(
                    f"{key}: alleen in GBS: {', '.join(alone)}\n    GBS:  {' / '.join(lines)}\n    lit:  {' / '.join(lit.get(v, []))}"
                    f"\n    ob:   {' / '.join(ob.get(v, []))}\n    1774: {' / '.join(old.get(v, []))}"
                )
            verses.append(lines)
        data[str(n)] = {"naam": f"Psalm {n}", "verzen": verses}

    head = [
        "Controle psalmen 1-150 (berijming 1773, GBS-uitgave); gemaakt door tools/bouw_psalmen_gbs.py",
        f"{sum(len(p['verzen']) for p in data.values())} verzen in {len(data)} psalmen.",
        "",
        "Aantal verzen per psalm:",
        *counts,
        "",
        "Besluiten (tools/psalm-correcties.json):",
    ]
    open(report_path, "w", encoding="utf-8").write("\n".join(head + report + (["", "OPEN:"] + problems if problems else [])) + "\n")
    if problems:
        print(f"{len(problems)} open punten; zie {report_path}")
        sys.exit(1)
    json.dump(data, open(out_path, "w", encoding="utf-8"), ensure_ascii=False)
    print(f"klaar: {sum(len(p['verzen']) for p in data.values())} verzen in {len(data)} psalmen")


if __name__ == "__main__":
    main(*sys.argv[1:])
