#!/usr/bin/env python3
"""Bouwt assets/data/belijdenissen.json en assets/data/formulieren.json en het controleverslag.

Eenmalige import; alle pagina's komen uit de cache van belhaal.py (max 1 verzoek/s).
Primaire bronnen:
  NGB  nederlandse-geloofsbelijdenis.nl (originele versie)   controle: Gihonbron-pdf (theologienet.nl)
  DL   dordtse-leerregels.nl                                  controle: theologienet.nl-pdf
  Formulieren  liturgie.nu (klassieke versie)                 controle: online-bijbel.nl/liturgie
  Apostolicum, Nicea, Athanasius  online-bijbel.nl            controle: zie verslag
Derde bron bij twijfel: online-bijbel.nl (NGB, DL) en de kerkboekuitgave van 1774 (DBNL).

Beslissingen staan in tools/belijdenis-oordelen.json:
  "correcties": [{doc, plaats, oud, nieuw, reden}]  -> toegepast op de primaire tekst
  "oordelen":   {"doc|plaats|A-lezing|B-lezing": "toelichting"}  -> per verschil
Verschillen zonder eigen oordeel die alleen spelling/verbuiging zijn, krijgen het oordeel
'vormvariant'; de overige zonder eigen oordeel 'versieverschil' (na handmatige controle).
"""
import json
import os
import re
import sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import bel_bronnen as B  # noqa: E402
import bel_vergelijk as V  # noqa: E402
from bel_tekst import netjes, verwijder_verwijzingen as vv, woorden  # noqa: E402

HIER = os.path.dirname(os.path.abspath(__file__))
DATA = os.path.join(HIER, "..", "assets", "data")
OORDELEN = os.path.join(HIER, "belijdenis-oordelen.json")
VERSLAG = os.path.join(HIER, "controle-belijdenissen.txt")


def laad_oordelen():
    if os.path.exists(OORDELEN):
        with open(OORDELEN) as f:
            return json.load(f)
    return {"correcties": [], "oordelen": []}


def pas_correcties_toe(doc, plaats, alineas, correcties, log):
    """alineas: lijst van strings (wordt aangepast). Elke correctie moet precies één keer passen."""
    for c in correcties:
        if c["doc"] != doc or str(c["plaats"]) != str(plaats):
            continue
        treffers = [(i, a.count(c["oud"])) for i, a in enumerate(alineas) if c["oud"] in a]
        if sum(n for _, n in treffers) != 1:
            raise SystemExit("Correctie past niet precies één keer: %r" % c)
        i = treffers[0][0]
        alineas[i] = netjes(alineas[i].replace(c["oud"], c["nieuw"]))
        c["_toegepast"] = True
        log.append(c)


def zoek_oordeel(oordelen, doc, plaats, v):
    a, b = " ".join(v.a).lower(), " ".join(v.b).lower()
    for i, o in enumerate(oordelen):
        if o["doc"] == doc and str(o.get("plaats", plaats)) == str(plaats) and o["a"].lower() == a and o["b"].lower() == b:
            return i, o["oordeel"]
    return None, None


class Verslag:
    def __init__(self):
        self.regels = []
        self.telling = {}

    def kop(self, t):
        self.regels += ["", "=" * 78, t, "=" * 78]

    def r(self, t=""):
        self.regels.append(t)

    def vergelijk(self, doc, plaats, label, a, b, oordelen, c=None):
        vs, lt, hl = V.vergelijk(a, b)
        t = self.telling.setdefault(doc, {"woord": 0, "vorm": 0, "leesteken": 0, "hoofdletter": 0,
                                          "fout_primair": 0, "fout_controle": 0, "versie": 0})
        t["leesteken"] += lt
        t["hoofdletter"] += hl
        uit = []
        for v in vs:
            k, o = zoek_oordeel(oordelen, doc, plaats, v)
            if k is not None:
                oordelen_gebruikt.add(k)
                if o.startswith("controlebron:"):
                    t["fout_controle"] += 1
                elif o.startswith("primair:"):
                    t["fout_primair"] += 1
                else:
                    t["versie"] += 1
                t["woord"] += 1
            elif V.is_vormvariant(v):
                o = "vormvariant (spelling/verbuiging), primaire lezing behouden"
                t["vorm"] += 1
            else:
                o = "versieverschil (andere uitgave), primaire lezing behouden"
                t["versie"] += 1
                t["woord"] += 1
            uit.append((v, o))
        if uit:
            self.r("%s  (%d woordverschil(len), %d leesteken, %d hoofdletter)" % (label, len(uit), lt, hl))
            for v, o in uit:
                self.r("    ...%s [A: %s] %s..." % (" ".join(v.links), " ".join(v.a) or "—", " ".join(v.rechts)))
                self.r("    ...%s [B: %s] %s..." % (" ".join(v.links), " ".join(v.b) or "—", " ".join(v.rechts)))
                self.r("       -> %s" % o)
        return uit

    def schrijf(self, pad):
        with open(pad, "w") as f:
            f.write("\n".join(self.regels).lstrip("\n") + "\n")


oordelen_gebruikt = set()


def main():
    cfg = laad_oordelen()
    corr, oordelen = cfg["correcties"], cfg["oordelen"]
    toegepast = []
    vs = Verslag()
    vs.r("CONTROLEVERSLAG belijdenissen.json en formulieren.json")
    vs.r("Gemaakt door tools/bouw_belijdenissen.py (eenmalige import).")
    vs.r("A = primaire bron (de tekst in de JSON), B = controlebron.")
    vs.r("Vergeleken na normaliseren van witruimte, aanhalingstekens, accenten en hoofdletters;")
    vs.r("bijbelverwijzingen zijn in beide versies eerst weggehaald. Leesteken- en hoofdletterverschillen")
    vs.r("worden per document alleen geteld. Elk woordverschil is nagelopen; het oordeel staat eronder:")
    vs.r("  vormvariant     = alleen spelling/verbuiging/aaneenschrijving (zichzelf/zichzelven, des/van den ...)")
    vs.r("  versieverschil  = andere uitgave met een ander (correct) woord; primaire lezing behouden")
    vs.r("  fouten in de primaire bron zijn in de JSON verbeterd; ze staan onder TOEGEPASTE CORRECTIES")
    vs.r("  (na verbetering verdwijnt het verschil uit de lijst per artikel)")
    vs.r("  controlebron: ... = fout/weglating in de controlebron; primaire lezing behouden")
    vs.r()
    vs.r("ALGEMENE BEVINDING. De controlebronnen (Gihonbron/theologienet, online-bijbel.nl) volgen een andere")
    vs.r("gangbare uitgave dan de primaire sites (o.a. 'zichzelven' i.p.v. 'zichzelf', 'dienaars' i.p.v.")
    vs.r("'dienaren', 'Gods' i.p.v. 'van God'). Daardoor zijn de meeste verschillen versie- of vormverschillen,")
    vs.r("geen fouten. Bij twijfel is de kerkboekuitgave van 1774 (DBNL) geraadpleegd; die staat in bijna alle")
    vs.r("gevallen aan de kant van de primaire bron. Alleen aantoonbare fouten (weglatingen, verbuigings- en")
    vs.r("tikfouten, afwijkende Schriftcitaten) zijn in de primaire tekst verbeterd.")

    # ------------------------------------------------------------ NGB
    ngb = B.ngb_primair()
    gihon, gtitels = B.ngb_gihon()
    ob_ngb = B.ngb_ob()
    vs.kop("NEDERLANDSE GELOOFSBELIJDENIS")
    vs.r("A: https://www.nederlandse-geloofsbelijdenis.nl/artikel-N (originele versie)")
    vs.r("B: Gihonbron-uitgave, https://theologienet.nl/bestanden/nederlandse-geloofsbelijdenis.pdf")
    vs.r("Derde bron bij twijfel: https://www.online-bijbel.nl/belijdenisgeschrift/nederlandse-geloofsbelijdenis/")
    for a in ngb:
        pas_correcties_toe("ngb", a["artikel"], a["tekst"], corr, toegepast)
    vs.r("Aantal artikelen: %d (Gihonbron: %d); alinea's: %d" % (len(ngb), len(gihon), sum(len(a["tekst"]) for a in ngb)))
    vs.r()
    for a in ngb:
        n = a["artikel"]
        vs.vergelijk("ngb", n, "Artikel %d" % n, " ".join(a["tekst"]), vv(gihon[n]), oordelen)

    # ------------------------------------------------------------ DL
    dl = B.dl_primair()
    dl_pdf = B.dl_pdf()
    vs.kop("DORDTSE LEERREGELS")
    vs.r("A: https://www.dordtse-leerregels.nl/hoofdstuk-N/paragraaf-N en /verwerping-N")
    vs.r("B: https://theologienet.nl/bestanden/dordtse-leerregels.pdf")
    vs.r("Derde bron bij twijfel: https://www.online-bijbel.nl/dordtseleerregels/")
    vs.r("Hoofdstuk III en IV vormen één hoofdstuk der leer; in de JSON als hoofdstuk 3 met een titel die")
    vs.r("dat aangeeft (zoals de bron: 'hoofdstuk-3-en-4'); hoofdstuk 4 bestaat daarom niet apart.")
    vs.r("De korte inleidende zin boven de verwerpingen ('De rechtzinnige leer ... verklaard zijnde, verwerpt")
    vs.r("de Synode de dwalingen dergenen:') staat niet op de primaire site en hoort niet bij een genummerd")
    vs.r("artikel; zij is niet opgenomen. Voorrede en Besluit vallen buiten de gevraagde vorm.")
    for h in dl:
        for soort, sl in (("paragraaf", "artikelen"), ("verwerping", "verwerpingen")):
            for x in h[sl]:
                pas_correcties_toe("dl", "%d-%s-%d" % (h["hoofdstuk"], soort[0], x["nummer"]), x["tekst"], corr, toegepast)
    for h in dl:
        vs.r("Hoofdstuk %s: %d artikelen, %d verwerpingen" % ("III/IV" if h["hoofdstuk"] == 3 else h["hoofdstuk"],
                                                          len(h["artikelen"]), len(h["verwerpingen"])))
    vs.r("Theologienet-pdf: %d genummerde stukken (moet 93 zijn)" % len(dl_pdf))
    vs.r()
    for h in dl:
        for soort, sl in (("paragraaf", "artikelen"), ("verwerping", "verwerpingen")):
            for x in h[sl]:
                k = (h["hoofdstuk"], soort, x["nummer"])
                hn = "III/IV" if h["hoofdstuk"] == 3 else str(h["hoofdstuk"])
                label = "Hoofdstuk %s, %s %d" % (hn, "artikel" if soort == "paragraaf" else "verwerping", x["nummer"])
                vs.vergelijk("dl", "%d-%s-%d" % (k[0], soort[0], k[2]), label, " ".join(x["tekst"]), vv(dl_pdf[k]), oordelen)

    # ------------------------------------------------------------ Formulieren
    vs.kop("LITURGISCHE FORMULIEREN")
    vs.r("A: https://www.liturgie.nu/formulieren/<id> (klassieke versie)")
    vs.r("B: https://www.online-bijbel.nl/liturgie/formulier/N (klassieke tekst, andere uitgave)")
    vs.r("Bij twijfel: kerkboek 1774 (DBNL, _boe008boek01_01_0179) voor de oorspronkelijke bewoording.")
    vs.r("Bijbelverwijzingen die in de zin staan en worden uitgesproken (bv. '1 Kor. 11:23-29', 'Psalm 103')")
    vs.r("zijn in de formulieren blijven staan; verwijzingen tussen haakjes en versnummers zijn weggelaten.")
    vs.r("Het doopformulier van liturgie.nu staat in het meervoud ('deze kinderen'), online-bijbel.nl in het")
    vs.r("enkelvoud ('dit kind'); dat geeft veel verschillen die alle 'versieverschil' zijn.")
    formulieren = []
    for fid, naam, kort, nr in B.FORMULIEREN:
        delen = B.formulier_primair(fid)
        platte = [d["kop"] for d in delen] + [t for d in delen for t in d["tekst"]]
        pas_correcties_toe("form:" + fid, "", platte, corr, toegepast)
        k = 0
        for d in delen:
            d["kop"] = platte[k]
            k += 1
        for d in delen:
            for j in range(len(d["tekst"])):
                d["tekst"][j] = platte[k]
                k += 1
        formulieren.append({"id": fid, "naam": naam, "kort": kort, "delen": delen})
        n_al = sum(len(d["tekst"]) for d in delen)
        vs.r()
        vs.r("--- %s (%s): %d delen, %d alinea's" % (naam, fid, len(delen), n_al))
        if nr is None:
            vs.r("    Geen tweede bron gevonden met deze tekst (belijdenisvragen ontleend aan Voetius); zie onder.")
            continue
        a = " ".join((d["kop"] + " " if d["kop"] else "") + " ".join(d["tekst"]) for d in delen)
        b = re.sub(r"(^| )\d+\. ", r"\1", " ".join(B.formulier_ob(nr)))
        vs.vergelijk("form:" + fid, "", "Hele formulier", vv(a), vv(b), oordelen)

    # ------------------------------------------------------------ Oecumenische belijdenissen
    vs.kop("APOSTOLISCHE GELOOFSBELIJDENIS, NICEA, ATHANASIUS")
    apost_items = B.creed_ob("12artikelen")
    apostolicum = maak_apostolicum(apost_items)
    nicea = [netjes(x) for x in B.creed_ob("nicea")]
    athanasius = [netjes(re.sub(r"^\d+\s+", "", x)) for x in B.creed_ob("athanasius")]
    for doc, tekst in (("apostolicum", apostolicum), ("nicea", nicea), ("athanasius", athanasius)):
        pas_correcties_toe(doc, "", tekst, corr, toegepast)
    hc = B.hc23()
    vs.r("A (alle drie): https://www.online-bijbel.nl/belijdenisgeschrift/{12artikelen,nicea,athanasius}/")
    vs.r("(belijdenis.nu en zustersites hebben deze drie belijdenissen niet.)")
    vs.r()
    vs.r("Apostolische Geloofsbelijdenis: 12 artikelen, als proza in %d alinea's." % len(apostolicum))
    hc23 = vind_hc23(hc)
    vs.r("B: Heidelberger Catechismus vraag 23 (assets/data/catechismus.json, Gihonbron-uitgave)")
    vs.vergelijk("apostolicum", "", "Apostolicum t.o.v. HC vraag 23", " ".join(apostolicum),
                 re.sub(r"\b\d+\b", " ", hc23), oordelen)
    vs.r()
    vs.r("Geloofsbelijdenis van Nicea: %d alinea's." % len(nicea))
    vs.r("Athanasius: %d genummerde zinnen, als %d alinea's." % (len(athanasius), len(athanasius)))
    vs.r("Voor Nicea en Athanasius is geen tweede bron gevonden met dezelfde (19e/20e-eeuwse) bewoording.")
    vs.r("Ter controle zijn ze zin voor zin naast de kerkboekuitgave van 1774 gelegd (DBNL _0181 en _0182);")
    vs.r("zie de handmatige bevindingen hieronder.")
    for regel in cfg.get("creed_notities", []):
        vs.r("  " + regel)

    # ------------------------------------------------------------ correcties en samenvatting
    vs.kop("TOEGEPASTE CORRECTIES OP DE PRIMAIRE TEKST")
    for c in toegepast:
        vs.r("%s %s: '%s' -> '%s'" % (c["doc"], c["plaats"], c["oud"], c["nieuw"]))
        vs.r("    reden: %s" % c["reden"])
    niet = [c for c in corr if not c.get("_toegepast")]
    if niet:
        raise SystemExit("Niet-toegepaste correcties: %r" % niet)
    ongebruikt = [oordelen[i] for i in range(len(oordelen)) if i not in oordelen_gebruikt]
    if ongebruikt:
        print("LET OP: ongebruikte oordelen:", *ongebruikt, sep="\n  ")

    vs.kop("SAMENVATTING")
    for doc, t in vs.telling.items():
        nc = sum(1 for c in toegepast if c["doc"] == doc)
        vs.r("%-36s verbeterd in primaire tekst %2d | resterende woordverschillen %3d (fout controlebron %d, "
             "versieverschil %d) | vormvarianten %3d | leesteken %3d | hoofdletter %3d"
             % (doc, nc, t["woord"], t["fout_controle"], t["versie"], t["vorm"], t["leesteken"], t["hoofdletter"]))
    vs.r("(athanasius: %d verbeteringen, zie hierboven; nicea: geen)" % sum(1 for c in toegepast if c["doc"] == "athanasius"))
    vs.r("Correcties toegepast: %d" % len(toegepast))
    vs.r("Weggehaalde bijbelverwijzingen (NGB, DL, haakjes in formulieren): %d" % len(B.REFLOG))

    # ------------------------------------------------------------ uitvoer
    bel = {
        "ngb": [{"artikel": a["artikel"], "titel": a["titel"], "tekst": a["tekst"]} for a in ngb],
        "dl": dl,
        "apostolicum": {"naam": "Apostolische Geloofsbelijdenis", "tekst": apostolicum},
        "nicea": {"naam": "Geloofsbelijdenis van Nicea", "tekst": nicea},
        "athanasius": {"naam": "Geloofsbelijdenis van Athanasius", "tekst": athanasius},
    }
    controleer_typografie(bel, formulieren)
    with open(os.path.join(DATA, "belijdenissen.json"), "w") as f:
        json.dump(bel, f, ensure_ascii=False, indent=1)
        f.write("\n")
    with open(os.path.join(DATA, "formulieren.json"), "w") as f:
        json.dump(formulieren, f, ensure_ascii=False, indent=1)
        f.write("\n")
    vs.schrijf(VERSLAG)
    print("klaar:", {d: t["woord"] for d, t in vs.telling.items()})


def maak_apostolicum(items):
    """De twaalf artikelen als doorlopende tekst in drie alinea's (Vader, Zoon, Geest)."""
    zinnen = [i.strip() for i in items]

    def klein(s):
        # Alleen het eerste woord verkleinen als het geen godsnaam/voornaamwoord voor God is.
        return s if s.startswith(("Die ", "Ik ")) else s[0].lower() + s[1:]

    vader = zinnen[0]
    zoon = zinnen[1] + " " + " ".join(klein(z) for z in zinnen[2:7])
    geest = zinnen[7] + " " + zinnen[8] + " " + " ".join(klein(z) for z in zinnen[9:12])
    return [netjes(vader), netjes(zoon), netjes(geest)]


def vind_hc23(hc):
    def zoek(o):
        if isinstance(o, dict):
            if o.get("vraag") == 23 or o.get("nummer") == 23:
                return o
            for v in o.values():
                r = zoek(v)
                if r:
                    return r
        elif isinstance(o, list):
            for v in o:
                r = zoek(v)
                if r:
                    return r
        return None
    v = zoek(hc)
    ant = v.get("antwoord") or v.get("tekst")
    return " ".join(ant) if isinstance(ant, list) else ant


def controleer_typografie(bel, formulieren):
    teksten = []
    for a in bel["ngb"]:
        teksten += a["tekst"]
    for h in bel["dl"]:
        for x in h["artikelen"] + h["verwerpingen"]:
            teksten += x["tekst"]
    for k in ("apostolicum", "nicea", "athanasius"):
        teksten += bel[k]["tekst"]
    for f in formulieren:
        for d in f["delen"]:
            teksten += d["tekst"]
    fout = []
    for t in teksten:
        if not t or t != t.strip() or "  " in t or re.search(r"\s[,.;:?!]", t) or re.search(r"[a-z][A-Z][a-z]", t) \
                or re.search(r"[,;:][A-Za-z]", t) or "<" in t or "&" in t:
            fout.append(t[:120])
    if fout:
        print("Typografie nalopen:", *fout, sep="\n  ")


if __name__ == "__main__":
    main()
