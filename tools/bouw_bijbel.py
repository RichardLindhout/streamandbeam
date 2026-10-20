"""Eenmalig: haalt de Statenvertaling (66 boeken, zonder apocriefen) van statenvertaling.net
en vergelijkt elk vers met een tweede bron (getbible.net).

Gebruik:  python3 tools/bouw_bijbel.py getbible-statenvertaling.json assets/data/statenvertaling.json controle-bijbel.txt

getbible.net mist trematekens (Bethanie i.p.v. Bethanië); die worden bij de vergelijking
genegeerd. Elk ander verschil komt in het controlebestand.
"""

import html
import json
import os
import re
import sys
import time
import unicodedata
import urllib.request

UA = {"User-Agent": "Mozilla/5.0 (streamandbeam; eenmalige import)"}
BASE = "https://www.statenvertaling.net"
CACHE = os.environ.get("CACHE", "/tmp/statenvertaling-cache")


def get(url):
    key = os.path.join(CACHE, re.sub(r"[^a-zA-Z0-9]+", "_", url))
    if os.path.exists(key):
        return open(key, encoding="utf-8").read()
    for attempt in range(4):
        try:
            page = urllib.request.urlopen(urllib.request.Request(url, headers=UA), timeout=30).read().decode("utf-8", "replace")
            os.makedirs(CACHE, exist_ok=True)
            open(key, "w", encoding="utf-8").write(page)
            time.sleep(0.2)
            return page
        except Exception:
            time.sleep(2 + attempt * 3)
    raise RuntimeError(f"kan {url} niet ophalen")


# Overview page of each book on statenvertaling.net, in canonical order.
BOOKS = """genesis exodus leviticus numeri deuteronomium jozua richteren ruth 1_samuel 2_samuel
1_koningen 2_koningen 1_kronieken 2_kronieken ezra nehemia esther job psalmen spreuken prediker
hooglied jesaja jeremia klaagliederen ezechiel daniel hosea joel amos obadja jona micha nahum
habakuk zefanja haggai zacharia maleachi mattheus marcus lukas johannes handelingen romeinen
1_korinthiers 2_korinthiers galaten efeziers filippensen kolossensen 1_tessalonicensen
2_tessalonicensen 1_timotheus 2_timotheus titus filemon hebreeen jakobus 1_petrus 2_petrus
1_johannes 2_johannes 3_johannes judas openbaring""".split()


def chapter_slug(book_page):
    text = get(f"{BASE}/bijbel/{book_page}.html")
    return re.search(r'href="(?:/bijbel/)?([a-z0-9]+)/1\.html', text).group(1)


def chapter(slug, n):
    text = get(f"{BASE}/bijbel/{slug}/{n}.html")
    verses = {}
    for num, body in re.findall(r'(?s)<p id="v(\d+)"><a name="\d+">\d+</a>(.*?)</p>', text):
        verses[int(num)] = re.sub(r"\s+", " ", html.unescape(re.sub(r"<[^>]+>", "", body))).strip()
    return verses


def plain(s):
    s = unicodedata.normalize("NFD", s)
    return re.sub(r"\s+", " ", "".join(c for c in s if not unicodedata.combining(c))).strip()


def main(getbible_path, out_path, report_path):
    other = {b["nr"]: b for b in json.load(open(getbible_path, encoding="utf-8"))["books"] if b["nr"] <= 66}
    pages = BOOKS
    report, data = [], []
    for nr, page in enumerate(pages, 1):
        ref = other[nr]
        slug = chapter_slug(page)
        chapters = []
        for c in ref["chapters"]:
            ours = chapter(slug, c["chapter"])
            theirs = {v["verse"]: v["text"].strip() for v in c["verses"]}
            if sorted(ours) != sorted(theirs):
                report.append(f"{ref['name']} {c['chapter']}: verzen {sorted(set(ours) ^ set(theirs))} komen maar in één bron voor")
            for v in sorted(set(ours) & set(theirs)):
                if plain(ours[v]) != plain(theirs[v]):
                    report.append(f"{ref['name']} {c['chapter']}:{v}\n  statenvertaling.net: {ours[v]}\n  getbible.net:        {theirs[v]}")
            chapters.append([ours[v] for v in sorted(ours)])
        data.append({"nummer": nr, "naam": ref["name"], "hoofdstukken": chapters})
        print(f"{nr} {ref['name']} ({slug}): {len(chapters)} hoofdstukken", flush=True)
    if len(data) != 66:
        report.append(f"LET OP: {len(data)} boeken gevonden in plaats van 66")
    json.dump(data, open(out_path, "w", encoding="utf-8"), ensure_ascii=False, separators=(",", ":"))
    open(report_path, "w", encoding="utf-8").write("\n".join(report) + "\n")
    print(f"klaar: {sum(len(c) for b in data for c in b['hoofdstukken'])} verzen, {len(report)} verschillen")


if __name__ == "__main__":
    main(*sys.argv[1:4])
