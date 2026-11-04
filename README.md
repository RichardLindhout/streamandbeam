# Stream & Beam

Eén programma voor de diensten van HHG Tholen: de liturgie op de TV's in de kerk en de
YouTube-uitzending (in plaats van PowerPoint en OBS).

## Bedienen

Open `http://<pc>:8080` op de PC, een laptop of een tablet.

1. **Dienst kiezen.** Bovenaan staan de diensten. **↻ Van website** haalt ze op van
   hhgtholen.nl: het programma leest de liturgie zelf (psalmen, lezingen, tekst, thema en
   punten). Met een `ANTHROPIC_API_KEY` leest Claude de pagina ook, als controle. Wat het
   programma niet kon plaatsen en elk verschil met Claude staan in een oranje blok
   (**Nakijken**); het programma houdt zijn eigen lezing aan. De predikant en de volgende
   dienst komen van kerktijden.nl. Elke dienst, ook op een andere dag, zet je op het scherm
   met **⋯ → Andere dienst** of in Voorbereiden met **Deze dienst op het scherm**. Met
   **Nieuwe dienst** (⋯ in Voorbereiden, of de instellingen) maak je er zelf een: één tik
   voor Morgendienst 09:30, Middagdienst 16:00 of Avonddienst 18:30, en met **Liturgie
   overnemen van** een kopie van een andere dienst, bijvoorbeeld om op woensdag te oefenen.
2. **Aanvullen of wijzigen.** Typ in het invoerveld wat er komt, bijvoorbeeld
   `ps 42:5, gebed, joh 3:16-21, HC 57, collecte`, en druk op Enter. Ook gezangen
   (`gez 3:1`, `LvM: 2`, `Morgenzang: 1`), `NGB art. 14`, `doopformulier` en `lied: …`.
   Zoals in de kerk is een streepje bij psalmen "en": `ps 27: 3-7` is vers 3 en 7; bij
   lezingen is het een reeks. Begrijpt het programma iets niet, dan staat er ⚠ met de
   reden (met een sleutel vraagt het eerst Claude). Sleep of gebruik ↑↓ voor de volgorde
   van de predikant.
3. **Vrij aanpassen.** ✎ bij een onderdeel toont de dia's als gewone tekst. Wat je daar
   typt, komt op de TV's en in de stream. **Herstel origineel** zet het terug.
4. **Tijdens de dienst:** **Volgende** (→, spatie of een presentatie-clicker). **Preek**,
   **Gebed** en **Slotzang** springen direct; **Zwart** maakt de TV's zwart.

De TV's openen `http://<pc>:8080/beam` en volgen de bediening vanzelf. Geef de PC een vast
IP-adres in de router.

**Roosters.** De collecte, de voorzang, de bijzondere dagen (Heilig Avondmaal, Biddag,
Kerst, …) en wie de stream bedient komen uit de Excel van de kerk (Streamupdate.xlsm). De
collecte van elke dienst komt uit het collecterooster; wat je zelf typt, blijft staan. Een
nieuwe dienst krijgt de voorzang uit het voorzangrooster (voor 13:00 de ochtend), en wijkt
de voorzang op de website af, dan staat dat bij de dienst onder *Nakijken*. De bijzondere
dag en "Bediening: naam" staan bij de dienst. Bijwerken: **Instellingen → Roosters → Roosters
uit Excel bijwerken**, of `streamandbeam importeer-excel Streamupdate.xlsm` (schrijft
`data/roosters.json`; zonder dat bestand geldt het ingebouwde rooster). In psalmen en
gezangen is een streepje "en": Ps. 27: 3-7 is vers 3 en 7.

## Teksten

Psalmen (berijming 1773 zoals de gemeente zingt), de Statenvertaling en de Heidelbergse
Catechismus zitten in het programma. Ze zijn eenmalig opgehaald en gecontroleerd tegen een
tweede bron; zie `assets/data/BRON.txt` en de scripts in `tools/`. Tijdens een dienst wordt
er niets van internet gehaald.

## YouTube-uitzending

Onder **Uitzending**: Voorbeeld, Alleen opnemen, Live en Stop. Elke klik op Volgende kiest
ook het beeld voor YouTube en draait de PTZOptics-camera naar de juiste positie:

| Onderdeel | Op YouTube | Camera |
|---|---|---|
| Welkom / Einde | kerkfoto met datum, predikant, volgende dienst, QR-code | overzicht |
| Psalm | eerst orgel met "Psalm 42 : 5" (voorspel), dan het vers met camera in de uitsparing | orgel |
| Lezing, Catechismus | tekst met camera in de uitsparing ("Zondag 22" eronder) | preekstoel |
| Collecte | doelen met QR-code | orgel |
| Gebed | camera op de doopvont met "Gebed" (TV's zwart) | doopvont |
| Prediking | eerst het thema met de punten, dan alleen de predikant | preekstoel |

**Automatisch live** (minuten vooraf) en **automatisch stoppen** (minuten na het eindscherm)
staan in de instellingen.

### Geluid

Kies de geluidsbron onder **⋯ → Instellingen → Geluid en camera**: de ingangen van de PC staan
in een lijst, de USB-geluidskaart van de mixer bovenaan. Daar zie je ook wat er nu binnenkomt,
en naast het beeldje bovenin beweegt een balkje mee met het geluid. Een nieuwe bron gaat meteen
in (tijdens live na een vraag; de uitzending hapert dan een paar seconden).

- **Rode balk**, ook in het voorbeeld: de gekozen bron is niet aangesloten of stuurt niets.
  Gaat de USB-kabel er weer in, dan komt het geluid vanzelf terug.
- **Rode balk tijdens live**: al 10 seconden stilte, of YouTube hoort niets.
- **Oranje melding**: stilte buiten live (tussen de diensten is dat normaal), of het geluid
  komt van de standaardingang van Windows terwijl de USB-geluidskaart er wel is.

### YouTube koppelen (eenmalig, ±15 minuten)

1. Ga naar https://console.cloud.google.com, maak een project ("Stream en Beam").
2. **API's en services → Bibliotheek**: zet de **YouTube Data API v3** aan.
3. **OAuth-toestemmingsscherm**: type *Extern*, vul de naam in, voeg het kerk-Google-account
   toe als testgebruiker, en zet de status daarna op **In productie** (anders vervalt de
   login na 7 dagen). Google noemt de app "niet geverifieerd"; voor eigen gebruik is dat
   normaal.
4. **Inloggegevens → OAuth-client-ID → Desktop-app**. Kopieer client-ID en -geheim naar de
   instellingen van Stream & Beam.
5. Open op de kerk-PC zelf `http://localhost:8080/youtube/login` en log in met het
   kanaalaccount.

Daarna maakt het programma bij **Live** zelf de uitzending aan, met titel en beschrijving uit
de liturgie, en sluit die af bij **Stop**. Zonder login werkt de streamsleutel uit de
instellingen.

## Installeren (Windows)

1. GStreamer 1.28 (MSVC 64-bit) van https://gstreamer.freedesktop.org/download/: de
   **runtime** én de **development** installer, optie "Complete".
2. Rust van https://rustup.rs.
3. `cargo build --release`, daarna `target\release\streamandbeam.exe`.
4. Niet nodig, wel handig: `ANTHROPIC_API_KEY`, voor de AI-controle van de website en hulp
   bij getypte onderdelen die het programma niet begrijpt. Zonder sleutel werkt alles,
   alleen zonder die controle.
5. Minder belasting: video-encoder `mfh264enc bitrate=4500` (elke videokaart),
   `nvh264enc bitrate=4500` (NVIDIA) of `qsvh264enc bitrate=4500` (Intel).

## Testen zonder kerk

`nepcamera` speelt de camera na met een opname van een eerdere dienst (`testdata/dienst.mp4`),
inclusief geluid, camera-posities en een voorbeeldwebsite. `testdienst` doet een complete
dienst automatisch en controleert alles: liturgie, live naar een nep-YouTube, alle dia's,
camera-posities, opname en de geluidswaarschuwing.

```sh
cargo build --release
target/release/testdienst            # --snel: kortere pauze per dia
target/release/testdienst --youtube  # echt live op het kanaal, privé, met TEST in de titel
```

Nodig: `ffmpeg`/`ffprobe`. `streamandbeam scenes` schrijft voorbeelden van de
YouTube-beelden naar `data/scenes`; `streamandbeam fetch` laat zien hoe het programma de
website leest (en met een sleutel wat de AI-controle anders leest). `cargo test` controleert
het lezen van de website met opgeslagen pagina's uit `fixtures/`.
