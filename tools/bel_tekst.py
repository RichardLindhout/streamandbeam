"""Gedeelde tekstfuncties: HTML naar nette tekst, bijbelverwijzingen weghalen, normaliseren."""
import html
import re
import unicodedata

BOEKEN = (r"Gen|Gn|Exod|Ex|Lev|Num|Deut|Deu|Joz|Richt|Ri|Ruth|Sam|Kon|Kron|Kr|Ezra|Neh|Esth|Est|Job|Ps|Psalm|Spr|Pred|"
          r"Hoogl|Jes|Jer|Klaagl|Ezech|Ez|Dan|Hos|Joël|Joel|Am|Amos|Ob|Jona|Micha|Mi|Nah|Hab|Zef|Hagg|Hag|Zach|Mal|"
          r"Matth|Matt|Math|Mat|Mt|Mark|Mk|Mr|Luk|Luc|Lk|Joh|Hand|Hd|Rom|Kor|Gal|Efeze|Efez|Ef|Filipp|Filip|Fil|Kol|Thess|Tess|Th|"
          r"Tim|Tit|Filem|Hebr|Heb|Jak|Petr|Pet|Judas|Jud|Openb|Op|Opb")
_VERS = r"\d+\s?:\s?\d+[a-z]?(?:\s?[-–]\s?\d+[a-z]?)?(?:\s?(?:[,;]|en)\s?(?:\d+\s?:\s?)?\d+[a-z]?(?:\s?[-–]\s?\d+[a-z]?)?)*"
_EEN = r"(?:[1-3]\s?)?(?:%s)\.?\s?(?:%s|\d+(?=[\s.,;:)]))" % (BOEKEN, _VERS)
# Een of meer verwijzingen, gescheiden door ; , of 'en', eventueel tussen haakjes; 'vgl.' en 'zie' mogen ervoor.
# Een verwijzing direct na 'in' ("In Gen. 1:26-27 zegt God", "de apostel in Hebr. 7:3") is zinsdeel en blijft staan.
REF = re.compile(r"\(\s*(?:vgl\.\s*|zie\s*)?%s(?:\s?(?:[;,]|en)\s?%s)*\s*\)|(?<![\w])(?<!\b[Ii]n )(?:vgl\.\s*)?%s(?:\s?(?:[;,]|en)\s?%s)*(?: en vervolgens)?" % (_EEN, _EEN, _EEN, _EEN))
# Volledig uitgeschreven namen die na 'de profeet' als naam blijven staan (alleen het vers gaat weg).
NAAM_VERS = re.compile(r"(\b(?:profeet|apostel) (?:Micha|Jona|Amos|Joël|Hosea|Jesaja|Jeremia|Ezechiël|Daniël|Habakuk|Zacharia|Maleachi|Petrus|Paulus|Johannes|Judas|Jakobus)) \d+:\d+[a-z]?(?:\s?[-–]\s?\d+[a-z]?)?")


def verwijder_verwijzingen(t, log=None):
    bron = t

    def weg(m):
        if log is not None:
            log.append("%s  [[%s]]  %s" % (bron[max(0, m.start() - 45):m.start()], m.group(0), bron[m.end():m.end() + 25]))
        return "\u0000"
    t = NAAM_VERS.sub(lambda m: (log.append("NAAM: " + m.group(0)) if log is not None else None) or m.group(1), t)
    bron = t
    t = REF.sub(weg, t)
    # Opruimen rond de weggehaalde plek.
    t = re.sub(r"\s*\u0000\s*", "\u0000", t)
    t = re.sub(r"\u0000+", "\u0000", t)
    t = re.sub(r"([,;])\u0000([:.?!;])", r"\2", t)            # ", Joh. 1:5:" -> ":"
    t = re.sub(r"([,;])\u0000([,])", r"\1", t)                 # ", Joh. 1:5," -> ","  en "; Ps. 69:5," -> ";"
    t = re.sub(r"([.?!’'\"”])\u0000\.", r"\1", t)              # "trekke? Joh. 6:44." -> "trekke?"
    t = re.sub(r"\u0000([.,;:?!])", r"\1", t)
    t = re.sub(r"([,;:])\u0000(?=[\s\w‘'(])", r"\1 ", t)
    t = re.sub(r"[,;]?\u0000$", ".", t)                         # verwijzing aan het eind van de alinea
    t = t.replace("\u0000", " ")
    t = re.sub(r"\(\s*\)", "", t)
    return netjes(t)


def netjes(t):
    t = t.replace("\xa0", " ").replace(" ", " ").replace("​", "")
    t = re.sub(r"\s+", " ", t).strip()
    t = re.sub(r"\s+([,.;:?!)])", r"\1", t)
    t = re.sub(r"\(\s+", "(", t)
    t = re.sub(r"([,;:])(?=[A-Za-zÀ-ÿ‘(])", r"\1 ", t)
    t = re.sub(r",\s*,", ",", t)
    t = re.sub(r"([?!])\s*[;,]", r"\1", t)
    t = re.sub(r"\s{2,}", " ", t)
    return t


def html_naar_tekst(frag):
    """Inline-HTML (zonder blokstructuur) naar tekst."""
    frag = re.sub(r"<a [^>]*class=\"(?:verwijzing|referentie)[^\"]*\"[^>]*>.*?</a>", " ", frag, flags=re.S)
    frag = re.sub(r"<span class=['\"]tekstnummer['\"]>.*?</span>", " ", frag, flags=re.S)
    frag = re.sub(r"<su[bp]>(.*?)</su[bp]>", r"\1", frag, flags=re.S)   # H<sub>EERE</sub> -> HEERE
    frag = re.sub(r"<br\s*/?>", " ", frag)
    frag = re.sub(r"<[^>]+>", "", frag)
    return netjes(html.unescape(frag))


def normaliseer(t):
    """Voor vergelijken: accenten, aanhalingstekens, streepjes en witruimte gelijkgetrokken."""
    t = unicodedata.normalize("NFKD", t)
    t = "".join(c for c in t if not unicodedata.combining(c))
    t = re.sub(r"[‘’‚‛´`'\"“”„«»]", "", t)
    t = re.sub(r"[–—‐-]", "-", t)
    t = re.sub(r"\s+", " ", t)
    return t.strip()


def woorden(t):
    """Woordtokens (met leestekens los) na normaliseren."""
    return re.findall(r"[\w-]+|[^\w\s]", normaliseer(t))
