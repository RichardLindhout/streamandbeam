"""Inlezen van alle bronnen (uit de cache van belhaal) naar eenvoudige structuren."""
import html
import json
import os
import re
import subprocess

import belhaal
from bel_tekst import html_naar_tekst, netjes, verwijder_verwijzingen

NGB = "https://www.nederlandse-geloofsbelijdenis.nl/artikel-%d"
NGB_PDF = "https://theologienet.nl/bestanden/nederlandse-geloofsbelijdenis.pdf"
DL = "https://www.dordtse-leerregels.nl/"
DL_PDF = "https://theologienet.nl/bestanden/dordtse-leerregels.pdf"
OB = "https://www.online-bijbel.nl"
LIT = "https://www.liturgie.nu/formulieren/"
DBNL = "https://www.dbnl.org/tekst/_boe008boek01_01/_boe008boek01_01_%04d.php"
HC23 = "https://www.heidelbergse-catechismus.nl/verlossing/zondag-7/vraag-23"

REFLOG = []  # (document, plaats, weggehaalde verwijzing)


def _zonder_refs(t, doc, plaats):
    log = []
    t = verwijder_verwijzingen(t, log)
    for r in log:
        REFLOG.append((doc, plaats, r))
    return t


def _zonder_haakjesrefs(t, doc, plaats):
    """Alleen verwijzingen tussen haakjes weghalen (formulieren: verwijzingen in de zin worden voorgelezen)."""
    from bel_tekst import REF
    def weg(m):
        if m.group(0).startswith("("):
            REFLOG.append((doc, plaats, m.group(0)))
            return ""
        return m.group(0)
    return netjes(REF.sub(weg, t))


def _pdftekst(url, layout=False):
    belhaal.haal(url, binary=True)
    args = ["pdftotext"] + (["-layout"] if layout else []) + [belhaal.pad(url), "-"]
    return subprocess.run(args, capture_output=True, text=True, check=True).stdout


def _inhoud(s, start_marker):
    """Neemt de HTML vanaf start_marker tot de bijbehorende sluitende </div>."""
    i = s.index(start_marker)
    diepte, j = 0, i
    for m in re.finditer(r"<div\b|</div>", s[i:]):
        diepte += 1 if m.group(0) == "<div" else -1
        if diepte == 0:
            j = i + m.end()
            break
    return s[i:j]


# ---------------------------------------------------------------- NGB
def ngb_primair():
    uit = []
    for n in range(1, 38):
        s = belhaal.haal(NGB % n)
        titel = re.search(r'<h1 class="item_titel"><span>Artikel \d+</span>(.*?)</h1>', s).group(1)
        b = _inhoud(s, '<div class="belijdenis_inhoud">')
        # Lijst (art. 18) hoort bij de voorgaande alinea: items met '; ' verbonden.
        blokken = re.findall(r"<p\b.*?</p>|<ul\b.*?</ul>", b, flags=re.S)
        alineas = []
        plak = False
        for blok in blokken:
            if blok.startswith("<ul"):
                items = [_zonder_refs(html_naar_tekst(li), "ngb", n) for li in re.findall(r"<li\b.*?</li>", blok, flags=re.S)]
                lijst = ""
                for it in items:
                    lijst += it if it[-1] in ",;" else it + ";"
                    lijst += " "
                alineas[-1] = alineas[-1] + " " + lijst.strip()
                plak = True
                continue
            t = _zonder_refs(html_naar_tekst(blok), "ngb", n)
            if plak:
                alineas[-1] = alineas[-1] + " " + t
                plak = False
            else:
                alineas.append(t)
        uit.append({"artikel": n, "titel": html_naar_tekst(titel), "tekst": [netjes(a) for a in alineas]})
    return uit


def ngb_gihon():
    t = _pdftekst(NGB_PDF)
    t = t[t.index("Artikel 1. Dat er een enig"):]
    regels = t.split("\n")
    art, titels, cur, n, voetnoot = {}, {}, [], None, False
    i = 0
    while i < len(regels):
        r = regels[i].strip().replace("\f", "")
        m = re.match(r"Artikel (\d+)\. (.*)", r)
        if m:
            if n:
                art[n] = cur
            n, cur, voetnoot = int(m.group(1)), [], False
            titel = m.group(2)
            while not titel.endswith(".") and i + 1 < len(regels) and regels[i + 1].strip():
                i += 1
                titel += " " + regels[i].strip()
            titels[n] = titel.rstrip(".")
        elif r.startswith("*"):
            voetnoot = True
        elif not r:
            voetnoot = False
        elif not voetnoot:
            cur.append(r)
        i += 1
    art[n] = cur
    out = {}
    for k, v in art.items():
        tekst = " ".join(v).replace("*", " ")
        tekst = re.sub(r"(\w)- (\w)", r"\1-\2", tekst)
        out[k] = netjes(tekst)
    return out, titels


def ngb_ob():
    s = belhaal.haal(OB + "/belijdenisgeschrift/nederlandse-geloofsbelijdenis/")
    s = re.sub(r"<div class=\"bewijsteksten-bron\".*?</div></div></div>", "", s, flags=re.S)
    s = re.sub(r"<button[^>]*>.*?</button>", "", s, flags=re.S)
    out = {}
    for sec in re.findall(r'<section class="artikel-eenheid">(.*?)</section>', s, flags=re.S):
        n = int(re.search(r"<span class='tekstnummer'>(\d+)</span>", sec).group(1))
        sec = re.sub(r"<h2.*?</h2>", "", sec, flags=re.S)
        out[n] = html_naar_tekst(sec)
    return out


# ---------------------------------------------------------------- DL
DL_TITELS = {
    1: "Het eerste hoofdstuk der leer: Van de Goddelijke verkiezing en verwerping",
    2: "Het tweede hoofdstuk der leer: Van den dood van Christus, en de verlossing der mensen door dezen",
    3: "Het derde en vierde hoofdstuk der leer: Van des mensen verdorvenheid en bekering tot God, en de manier van deze",
    5: "Het vijfde hoofdstuk der leer: Van de volharding der heiligen",
}
DL_SLUG = {1: "hoofdstuk-1", 2: "hoofdstuk-2", 3: "hoofdstuk-3-en-4", 5: "hoofdstuk-5"}


def dl_primair():
    s = belhaal.haal(DL)
    links = set(re.findall(r'href="https://www.dordtse-leerregels.nl/(hoofdstuk-[^"/]+/(?:paragraaf|verwerping)-\d+)"', s))
    uit = []
    for h in (1, 2, 3, 5):
        hoofd = {"hoofdstuk": h, "titel": DL_TITELS[h], "artikelen": [], "verwerpingen": []}
        for soort, sleutel in (("paragraaf", "artikelen"), ("verwerping", "verwerpingen")):
            nrs = sorted(int(l.rsplit("-", 1)[1]) for l in links if l.startswith(DL_SLUG[h] + "/" + soort))
            for nr in nrs:
                p = belhaal.haal(DL + "%s/%s-%d" % (DL_SLUG[h], soort, nr))
                b = _inhoud(p, '<div class="belijdenis_inhoud">')
                als = [_zonder_refs(html_naar_tekst(x), "dl", (h, soort, nr)) for x in re.findall(r"<p\b.*?</p>", b, flags=re.S)]
                hoofd[sleutel].append({"nummer": nr, "tekst": [a for a in als if a]})
        uit.append(hoofd)
    return uit


def dl_pdf():
    t = _pdftekst(DL_PDF)
    t = t[t.index("HOOFDSTUK 1"):]
    t = re.sub(r"\n\s*HOOFDSTUK [^\n]*(?:\n[^\n0-9]*?)??(?=\n\d)", "\n", t)
    t = re.sub(r"Verwerping der dwalingen[^\n]*\n(?:[^\n]*\n)*?(?=\s*\d)", "\n", t)
    stukken = re.split(r"(?:^|\n)\s*(\d(?:/4)?)-(\d+)(a?)\.\s*", t)
    out = {}
    for i in range(1, len(stukken) - 3, 4):
        h = 3 if stukken[i] == "3/4" else int(stukken[i])
        key = (h, "verwerping" if stukken[i + 2] else "paragraaf", int(stukken[i + 1]))
        tekst = re.sub(r"\s+", " ", stukken[i + 3])
        tekst = re.split(r"\s(?:HOOFDSTUK \d|Verwerping der dwalingen)", tekst)[0]
        out[key] = netjes(tekst)
    return out


def dl_ob():
    out = {}
    for h, nr in ((1, 1), (2, 2), (3, 3), (5, 5)):
        for deel, soort in (("deel1", "paragraaf"), ("deel2", "verwerping")):
            s = belhaal.haal(OB + "/dordtseleerregels/hoofdstuk/%d/%s/" % (nr, deel))
            art = s[s.index("<article"):s.index("</article>")]
            for v, li in re.findall(r'<li value="(\d+)">(.*?)</li>', art, flags=re.S):
                out[(h, soort, int(v))] = html_naar_tekst(li)
    return out


# ---------------------------------------------------------------- Formulieren
FORMULIEREN = [
    ("doop-kinderen", "Formulier om de Heilige Doop te bedienen aan de kleine kinderen der gelovigen", "Doopformulier", 1),
    ("doop-volwassenen", "Formulier om de Heilige Doop te bedienen aan volwassen personen", "Doopformulier volwassenen", 2),
    ("heilig-avondmaal", "Formulier om het Heilig Avondmaal te houden", "Avondmaalsformulier", 3),
    ("belijdenis", "Formulier voor het afleggen van openbare geloofsbelijdenis", "Formulier openbare belijdenis", None),
    ("bevestiging-ouderling-diaken", "Formulier om ouderlingen en diakenen te bevestigen", "Bevestiging ouderlingen en diakenen", 5),
    ("bevestiging-predikant", "Formulier om de dienaren des Goddelijken Woords te bevestigen", "Bevestiging predikant", 4),
    ("huwelijk", "Formulier om den huwelijken staat voor de gemeente van Christus te bevestigen", "Huwelijksformulier", 6),
    ("christelijke-ban", "Formulier van den ban of de afsnijding van de gemeente van Christus", "Formulier van de ban", 7),
    ("wederopneming-in-de-gemeente", "Formulier van wederopneming des afgesnedenen in de gemeente van Christus", "Wederopneming", 8),
]


def formulier_primair(fid):
    """Klassieke versie van liturgie.nu als lijst van (kop, [alinea's])."""
    s = belhaal.haal(LIT + fid)
    i = s.index('<div class="formulierversie klassiek">')
    j = s.index('<div class="formulierversie', i + 10)
    b = s[i:j]
    b = re.sub(r"<h3>.*?</h3>", "", b, flags=re.S)
    b = re.sub(r"\s*<a href=\"#bewijs[^>]*>.*?</a>\s*", " ", b, flags=re.S)
    # Blokstructuur: p, li en br worden alinea-grenzen; <strong> aan het begin van een regel is een kop.
    b = re.sub(r"<li\b[^>]*>", "\n\u0001BLOK", b)
    b = re.sub(r"</?(?:p|ul|ol|li|div)\b[^>]*>", "\n\u0001BLOK", b)
    b = re.sub(r"<br\s*/?>", "\n", b)
    delen = [["", []]]
    nieuw_blok = True
    for regel in b.split("\n"):
        if "\u0001BLOK" in regel:
            nieuw_blok = True
            regel = regel.replace("\u0001BLOK", "")
        if not html_naar_tekst(regel):
            continue
        m = re.match(r"\s*<strong>(.*?)</strong>(.*)$", regel, flags=re.S)
        if m and html_naar_tekst(m.group(2)) == "":
            delen.append([html_naar_tekst(m.group(1)), []])
            nieuw_blok = True
            continue
        tekst = html_naar_tekst(regel)
        tekst = re.sub(r"^\d+\.\s+", "", tekst)               # versnummers van Ps. 103
        tekst = re.sub(r"(\d+) :(\d)", r"\1:\2", tekst)
        tekst = _zonder_haakjesrefs(tekst, "form:" + fid, len(delen))
        alineas = delen[-1][1]
        # Een <br> binnen een blok splitst alleen na een zinseinde; lopende regels (Onze Vader) blijven bijeen.
        if alineas and not nieuw_blok and (not re.search(r"[.:?!’]$", alineas[-1]) or re.match(r"[a-z]", tekst)
                                           or alineas[-1].startswith("Onze Vader")):
            alineas[-1] = alineas[-1] + " " + tekst
        else:
            alineas.append(tekst)
        nieuw_blok = False
    return [{"kop": k, "tekst": t} for k, t in delen if t]


def formulier_ob(nr):
    s = belhaal.haal(OB + "/liturgie/formulier/%d/" % nr)
    art = s[s.index("<article"):s.index("</article>")]
    art = re.sub(r"<br\s*/?>", "\n", art)
    return [html_naar_tekst(x) for x in art.split("\n") if html_naar_tekst(x)]


def dbnl(nr):
    s = belhaal.haal(DBNL % nr)
    s = re.sub(r"<script.*?</script>|<style.*?</style>", "", s, flags=re.S)
    s = re.sub(r"</p>|<br\s*/?>|</h\d>", "\n", s)
    return "\n".join(html_naar_tekst(x) for x in s.split("\n") if html_naar_tekst(x))


# ---------------------------------------------------------------- Oecumenische belijdenissen
def creed_ob(slug):
    s = belhaal.haal(OB + "/belijdenisgeschrift/%s/" % slug)
    art = s[s.index("<article"):s.index("</article>")]
    if "<li>" in art:
        return [html_naar_tekst(x) for x in re.findall(r"<li>(.*?)</li>", art, flags=re.S)]
    art = re.sub(r"<br\s*/?>", "\n", art)
    return [html_naar_tekst(x) for x in re.findall(r"<p>(.*?)</p>", art, flags=re.S)]


def hc23():
    base = os.path.join(os.path.dirname(__file__), "..", "assets", "data", "catechismus.json")
    with open(base) as f:
        hc = json.load(f)
    return hc
