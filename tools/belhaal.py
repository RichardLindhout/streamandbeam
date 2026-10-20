"""Beleefd ophalen met cache (max 1 verzoek/s)."""
import hashlib, os, sys, time, urllib.request

CACHE = "/private/tmp/claude-501/-Users-richardlindhout-go-src-github-com-RichardLindhout-streamandbeam/f83737d1-aa99-4f80-8b62-04b75e83f77b/scratchpad/belcache"
UA = "Mozilla/5.0 (streamandbeam; eenmalige import)"
_last = [0.0]

def pad(url):
    h = hashlib.sha1(url.encode()).hexdigest()[:16]
    ext = ".pdf" if url.lower().endswith(".pdf") else ".html"
    return os.path.join(CACHE, h + ext)

def haal(url, binary=False):
    os.makedirs(CACHE, exist_ok=True)
    p = pad(url)
    if not os.path.exists(p):
        wacht = 1.1 - (time.time() - _last[0])
        if wacht > 0:
            time.sleep(wacht)
        req = urllib.request.Request(url, headers={"User-Agent": UA})
        try:
            with urllib.request.urlopen(req, timeout=60) as r:
                data = r.read()
        finally:
            _last[0] = time.time()
        with open(p, "wb") as f:
            f.write(data)
        with open(os.path.join(CACHE, "index.txt"), "a") as f:
            f.write(os.path.basename(p) + " " + url + "\n")
    data = open(p, "rb").read()
    return data if binary else data.decode("utf-8", "replace")

if __name__ == "__main__":
    for u in sys.argv[1:]:
        print(pad(u), len(haal(u, binary=True)))
