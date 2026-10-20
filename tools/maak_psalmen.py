"""Stap 2: maakt assets/data/psalmen.json uit de opgehaalde pagina's (zie bouw_psalmen.py,
die alles in de cache zet) en controleert elk vers woord voor woord.

Bron is psalmboek.nl: die tekst en leestekens komen overeen met wat de gemeente zingt.
Elk vers wordt vergeleken met online-bijbel.nl; waar de woorden verschillen staat ook de
uitgave van 1774 (DBNL) erbij. Over elk verschil is één keer een besluit genomen in
tools/psalm-correcties.json:

    "18:13": {"neergehogen": "neergebogen"}     woord vervangen in dit vers
    "18:2":  {"=": "hoog/hoogst: psalmboek klopt, zoals 1774"}   bewust zo gelaten

Het script stopt als er een verschil is zonder besluit, zodat er nooit een ongecontroleerd
vers in het programma komt.

Gebruik:  python3 tools/maak_psalmen.py assets/data/psalmen.json [open-verschillen.txt]
"""

import difflib
import json
import re
import sys
import unicodedata

sys.argv, ARGS = sys.argv[:1], sys.argv[1:]
exec(open("tools/bouw_psalmen.py", encoding="utf-8").read().split("if __name__")[0])


def clean(lines):
    """psalmboek.nl splits some words for the melody ("vol - ken")."""
    return [re.sub(r"(\w) - (\w)", r"\1\2", l) for l in lines]


def tokens(lines):
    s = unicodedata.normalize("NFD", " ".join(lines).lower().replace("’", "'"))
    s = "".join(c for c in s if not unicodedata.combining(c)).replace("'", "")
    return re.findall(r"[a-z]+", s)


def apply(lines, fixes):
    out = []
    for l in lines:
        for wrong, right in fixes.items():
            if wrong != "=":
                l = re.sub(rf"(?<![\w']){re.escape(wrong)}(?![\w'])", right, l)
        out.append(l)
    return out


def main(out_path, open_path=None):
    corrections = json.load(open("tools/psalm-correcties.json", encoding="utf-8"))
    data, open_diffs = {}, []
    for n in range(1, 161):
        title, first, count = psalmboek_verse(n, 1)
        name = re.sub(r"\s+vers\s+\d+$", "", title).strip()
        verses = [clean(first)] + [clean(psalmboek_verse(n, v)[1]) for v in range(2, count + 1)]
        second = online_bijbel(n) if n <= 150 else {}
        old = dbnl_verses(n) if n <= 150 else []
        for i, lines in enumerate(verses, 1):
            key = f"{n}:{i}"
            fixes = corrections.get(key, {})
            lines = verses[i - 1] = apply(lines, fixes)
            if n > 150 or "=" in fixes:
                continue
            a, b = tokens(lines), tokens(second.get(i, []))
            if a != b:
                diff = [
                    f"{' '.join(a[i1:i2]) or '—'} ↔ {' '.join(b[j1:j2]) or '—'}"
                    for t, i1, i2, j1, j2 in difflib.SequenceMatcher(None, a, b, autojunk=False).get_opcodes()
                    if t != "equal"
                ]
                open_diffs.append(
                    f"{key}  {'; '.join(diff)}\n  psalmboek:     {' / '.join(lines)}\n"
                    f"  online-bijbel: {' / '.join(second.get(i, []))}\n  1774:          {' / '.join(old[i - 1]) if i <= len(old) else '-'}"
                )
        data[str(n)] = {"naam": name, "verzen": verses}
    if open_diffs:
        text = "\n".join(open_diffs) + "\n"
        if open_path:
            open(open_path, "w", encoding="utf-8").write(text)
        print(f"{len(open_diffs)} verzen met een verschil zonder besluit; zie {open_path or 'hieronder'}")
        if not open_path:
            print(text)
        sys.exit(1)
    json.dump(data, open(out_path, "w", encoding="utf-8"), ensure_ascii=False, indent=1)
    print(f"klaar: {sum(len(p['verzen']) for p in data.values())} verzen, alle verschillen beslist")


if __name__ == "__main__":
    main(*ARGS)
