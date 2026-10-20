"""Eenmalig: haalt de psalmen (berijming 1773) en enige gezangen van psalmboek.nl en
controleert ze tegen de oorspronkelijke uitgave (DBNL, 1774).

Gebruik:  python3 tools/bouw_psalmen.py assets/data/psalmen.json controle-psalmen.txt

Controles per psalm: zelfde aantal verzen als DBNL, per vers zelfde aantal regels, en
woordgelijkenis (spelling verschilt, maar de woorden horen grotendeels gelijk te zijn).
Afwijkingen komen in het controlebestand, zodat een mens ze kan nalopen.
"""

import difflib
import html
import json
import os
import re
import sys
import time
import urllib.request

UA = {"User-Agent": "Mozilla/5.0 (streamandbeam; eenmalige import)"}


CACHE = os.environ.get("CACHE", "/tmp/psalmboek-cache")


def get(url):
    key = os.path.join(CACHE, re.sub(r"[^a-zA-Z0-9]+", "_", url))
    if os.path.exists(key):
        return open(key, encoding="utf-8").read()
    # Patient: psalmboek.nl blocks visitors who ask too fast, so wait and keep trying.
    for attempt in range(40):
        try:
            time.sleep(4 if "psalmboek" in url else 1)
            req = urllib.request.Request(url, headers=UA)
            page = urllib.request.urlopen(req, timeout=30).read().decode("utf-8", "replace")
            os.makedirs(CACHE, exist_ok=True)
            if "tekstpsalmvers" in page or "dbnl" in url:
                open(key, "w", encoding="utf-8").write(page)
            return page
        except Exception:
            print(f"  wacht op {url} (poging {attempt + 1})", flush=True)
            time.sleep(300)
    raise RuntimeError(f"kan {url} niet ophalen")


def psalmboek_verse(n, v):
    page = get(f"https://psalmboek.nl/zingen.php?psalm={n}&psvID={v}")
    title = re.search(r"<h1>(.*?)</h1>", page).group(1)
    m = re.search(r"(?s)<p class='tekstpsalmvers'>(.*?)</p>", page)
    if not m:
        open(os.path.join(CACHE, f"FOUT-{n}-{v}.html"), "w", encoding="utf-8").write(page)
        raise RuntimeError(f"geen verstekst op psalm {n} vers {v}")
    body = m.group(1)
    lines = [html.unescape(re.sub(r"<[^>]+>", "", l)).strip() for l in re.split(r"<br\s*/?>", body)]
    # The verse links; "0" is the psalm's introduction, not a verse.
    numbers = {int(x) for x in re.findall(rf'href="\?psalm={n}&psvID=(\d+)#psvs"', page)} | {v}
    return title, [l for l in lines if l], max(x for x in numbers if x > 0)


def dbnl_verses(n):
    page = get(f"https://www.dbnl.org/tekst/_boe008boek01_01/_boe008boek01_01_{12 + n:04d}.php")
    parts = re.split(r"<h4>&sect;\d+</h4>", page)[1:]
    if parts:
        parts[-1] = parts[-1].split('id="right"')[0]
    return [
        [html.unescape(re.sub(r"<[^>]+>", "", l)).strip() for l in re.findall(r'<div class="line-content">(.*?)</div>', p)]
        for p in parts
    ]


def online_bijbel(n):
    """The same psalm from online-bijbel.nl, as a second source for the words."""
    page = get(f"https://www.online-bijbel.nl/psalm/{n}/")
    verses = {}
    for num, body in re.findall(r"(?s)data-vers='(\d+)'.*?</span>\s*(.*?)</p>", page):
        lines = [html.unescape(re.sub(r"<[^>]+>", "", l)).strip() for l in re.split(r"<br\s*/?>", body)]
        lines = [l for l in lines if l]
        if lines:
            lines[0] = re.sub(r"^\d+\s+", "", lines[0])  # the verse number
        verses[int(num)] = lines
    return verses


def bare_words(lines):
    return re.findall(r"[a-zëïéèêôûü']+", " ".join(lines).lower().replace("’", "'"))


def words(lines):
    return re.findall(r"[a-zë']+", " ".join(lines).lower().replace("ee", "e").replace("oo", "o").replace("aa", "a"))


def main(out_path, report_path):
    data, report = {}, []
    for n in range(1, 161):
        title, first, count = psalmboek_verse(n, 1)
        verses = [first]
        for v in range(2, count + 1):
            verses.append(psalmboek_verse(n, v)[1])
            time.sleep(0.25)
        name = re.sub(r"\s+vers\s+\d+$", "", title).strip()
        data[str(n)] = {"naam": name, "verzen": verses}
        if n <= 150:
            second = online_bijbel(n)
            if len(second) != len(verses):
                report.append(f"{name}: {len(verses)} verzen, online-bijbel.nl heeft er {len(second)}")
            for i, a in enumerate(verses, 1):
                b = second.get(i, [])
                if bare_words(a) != bare_words(b):
                    report.append(f"{name}:{i}: woorden verschillen met online-bijbel.nl\n  psalmboek:    {' / '.join(a)}\n  online-bijbel: {' / '.join(b)}")
            old = dbnl_verses(n)
            if len(old) != len(verses):
                report.append(f"{name}: {len(verses)} verzen, DBNL heeft er {len(old)}")
            for i, (a, b) in enumerate(zip(verses, old), 1):
                if len(a) != len(b):
                    report.append(f"{name}:{i}: {len(a)} regels, DBNL {len(b)}")
                ratio = difflib.SequenceMatcher(None, words(a), words(b)).ratio()
                if ratio < 0.8:
                    report.append(f"{name}:{i}: woorden wijken af (gelijkenis {ratio:.2f})\n  nu:   {' / '.join(a)}\n  1774: {' / '.join(b)}")
        print(f"{name}: {len(verses)} verzen", flush=True)
    with open(out_path, "w", encoding="utf-8") as f:
        json.dump(data, f, ensure_ascii=False, indent=1)
    with open(report_path, "w", encoding="utf-8") as f:
        f.write("\n".join(report) + "\n")
    print(f"klaar: {sum(len(p['verzen']) for p in data.values())} verzen, {len(report)} opmerkingen")


if __name__ == "__main__":
    main(sys.argv[1], sys.argv[2])
