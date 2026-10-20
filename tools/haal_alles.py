"""Haalt alle bronpagina's op (in cache) voor bouw_belijdenissen.py."""
import re, belhaal
urls = ["https://www.nederlandse-geloofsbelijdenis.nl/artikel-%d" % i for i in range(1, 38)]
s = belhaal.haal("https://www.dordtse-leerregels.nl/")
dl = sorted(set(re.findall(r'href="(https://www.dordtse-leerregels.nl/hoofdstuk-[^"]+)"', s)))
urls += dl
urls += ["https://www.online-bijbel.nl/dordtseleerregels/",
         "https://www.dbnl.org/tekst/_boe008boek01_01/_boe008boek01_01_0179.php",
         "https://www.dbnl.org/tekst/_boe008boek01_01/_boe008boek01_01_0181.php",
         "https://www.dbnl.org/tekst/_boe008boek01_01/_boe008boek01_01_0182.php",
         "https://www.dbnl.org/tekst/_boe008boek01_01/_boe008boek01_01_0168.php",
         "https://www.heidelbergse-catechismus.nl/verlossing/zondag-7/vraag-23"]
for u in urls:
    try:
        belhaal.haal(u)
    except Exception as e:
        print("FOUT", u, e)
print(len(urls), "klaar")
