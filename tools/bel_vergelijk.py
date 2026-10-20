"""Woordvergelijking tussen twee (of drie) versies van een tekst."""
import difflib
import re

from bel_tekst import woorden

PUNCT = re.compile(r"^[^\w]+$")


def _w(tokens):
    return [t for t in tokens if not PUNCT.match(t)]


class Verschil:
    def __init__(self, soort, a, b, links, rechts):
        self.soort, self.a, self.b, self.links, self.rechts = soort, a, b, links, rechts
        self.oordeel = None

    def sleutel(self):
        return (" ".join(self.a).lower(), " ".join(self.b).lower())


def vergelijk(tekst_a, tekst_b):
    """Geeft (lijst woordverschillen, aantal leestekenverschillen, aantal hoofdletterverschillen)."""
    ta, tb = woorden(tekst_a), woorden(tekst_b)
    la, lb = [t.lower() for t in ta], [t.lower() for t in tb]
    sm = difflib.SequenceMatcher(None, la, lb, autojunk=False)
    verschillen, leesteken, hoofdletter = [], 0, 0
    for op, i1, i2, j1, j2 in sm.get_opcodes():
        if op == "equal":
            hoofdletter += sum(1 for k in range(i2 - i1) if ta[i1 + k] != tb[j1 + k])
            continue
        a, b = ta[i1:i2], tb[j1:j2]
        if not _w(a) and not _w(b):
            leesteken += 1
            continue
        # Alleen de woorden tellen; leestekens binnen het stuk zijn bijzaak.
        if [x.lower() for x in _w(a)] == [x.lower() for x in _w(b)]:
            leesteken += 1
            continue
        links = _w(ta[max(0, i1 - 12):i1])[-5:]
        rechts = _w(ta[i2:i2 + 12])[:5]
        verschillen.append(Verschil("woord", _w(a), _w(b), links, rechts))
    return verschillen, leesteken, hoofdletter


def komt_voor(c_tekst, links, midden, rechts, n=2):
    """Staat 'links midden rechts' (n woorden context) letterlijk in tekst C?"""
    c = " " + " ".join(x.lower() for x in _w(woorden(c_tekst))) + " "
    zoek = " ".join(x.lower() for x in (links[-n:] + midden + rechts[:n]))
    return (" " + zoek + " ") in c


def arbitreer(v, c_tekst):
    """Stemt met een derde bron: 'A', 'B', 'beide', of 'geen'."""
    ia = komt_voor(c_tekst, v.links, v.a, v.rechts)
    ib = komt_voor(c_tekst, v.links, v.b, v.rechts)
    if ia and not ib:
        return "A"
    if ib and not ia:
        return "B"
    # Met minder context nog eens proberen.
    ia = komt_voor(c_tekst, v.links, v.a, v.rechts, 1)
    ib = komt_voor(c_tekst, v.links, v.b, v.rechts, 1)
    if ia and not ib:
        return "A"
    if ib and not ia:
        return "B"
    return "beide" if ia else "geen"


# ------------------------------------------------------------------ vormvarianten
_FRASEN = [
    (r"\bvan (?:den|het) (\w+)", r"des \1"), (r"\bvan de (\w+)", r"der \1"),
    (r"\bvan god\b", "gods"), (r"\bvan uw\b", "uws"), (r"\buwer\b", "uws"), (r"\bvan zijn\b", "zijns"),
    (r"\bvan onzen?\b", "onzes"), (r"\bvan node\b", "nodig"), (r"\b(?:het begin|den beginne)\b", "begin"),
    (r"\bt\b", "het"), (r"\bzoude\b", "zou"), (r"\bzone\b", "zoon"), (r"\bveel meer\b", "veelmeer"),
]
_KORT = {frozenset(p) for p in [("de", "den"), ("die", "dien"), ("ere", "eer"), ("had", "hadt"), ("ziet", "zie"),
                                ("vrees", "vreze"), ("eer", "ere"), ("ten", "te"), ("zelf", "zelve"), ("zelf", "zelven")]}
_SUFFIX = ("n", "e", "en", "s", "es", "lijk", "elijk", "t", "ve", "ven")


def _canon_tekst(ws):
    t = " ".join(w.lower() for w in ws)
    t = t.replace("th", "t").replace("c", "k")
    for p, r in _FRASEN:
        t = re.sub(p, r, t)
    return t


def _woord_gelijk(x, y):
    if x == y or frozenset((x, y)) in _KORT:
        return True
    a, b = sorted((x, y), key=len)
    if len(a) >= 4 and b.startswith(a) and b[len(a):] in _SUFFIX:
        return True
    # zichzelf/zichzelven, onszelf/onszelven, uzelf/uzelven
    if a.endswith("zelf") and b.startswith(a[:-1]) and b[len(a) - 1:] in ("ve", "ven"):
        return True
    # dienaren/dienaars, herderen/herders, conscientien/conscienties
    for s1, s2 in (("aren", "aars"), ("eren", "ers"), ("ien", "ies")):
        if (x.endswith(s1) and y.endswith(s2) and x[:-len(s1)] == y[:-len(s2)]) or \
           (y.endswith(s1) and x.endswith(s2) and y[:-len(s1)] == x[:-len(s2)]):
            return True
    if len(a) >= 5 and b.startswith(a[:-1]) and (a.endswith("ig") or a.endswith("e")) and b.endswith("lijk"):
        return True  # getrouw/getrouwelijk, eendrachtiglijk/eendrachtelijk
    return False


def is_vormvariant(v):
    """Alleen spelling/verbuiging/aaneenschrijving; geen ander woord."""
    a, b = _canon_tekst(v.a), _canon_tekst(v.b)
    if a.replace(" ", "").replace("-", "") == b.replace(" ", "").replace("-", ""):
        return True
    ta, tb = a.split(), b.split()
    return len(ta) == len(tb) and len(ta) > 0 and all(_woord_gelijk(x, y) for x, y in zip(ta, tb))
