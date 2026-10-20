# Stream & Beam

Eén programma voor de diensten van HHG Tholen: de liturgie op de TV's in de kerk en de
YouTube-uitzending (in plaats van PowerPoint en OBS).

## Bedienen

Open `http://<pc>:8080` op de PC, een laptop of een tablet.

1. **Dienst kiezen.** Bovenaan staan de diensten. **↻ Van website** haalt ze op van
   hhgtholen.nl: Claude leest de liturgie en zet die in een vast formaat. Wat Claude niet
   kon plaatsen, staat in een oranje blok. De predikant en de volgende dienst komen van
   kerktijden.nl. Met **+ Nieuwe dienst** maak je er zelf een.
2. **Aanvullen of wijzigen.** Typ in het invoerveld wat er komt, bijvoorbeeld
   `ps 42:5, gebed, joh 3:16-21, HC 57, collecte`, en druk op Enter. Twijfelt het
   programma, dan staat er ⚠ met de reden. Sleep of gebruik ↑↓ voor de volgorde van de
   predikant.
3. **Vrij aanpassen.** ✎ bij een onderdeel toont de dia's als gewone tekst. Wat je daar
   typt, komt op de TV's en in de stream. **Herstel origineel** zet het terug.
4. **Tijdens de dienst:** **Volgende** (→, spatie of een presentatie-clicker). **Preek**,
   **Gebed** en **Slotzang** springen direct; **Zwart** maakt de TV's zwart.

De TV's openen `http://<pc>:8080/beam` en volgen de bediening vanzelf. Geef de PC een vast
IP-adres in de router.

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

Een rode balk waarschuwt als er geen geluid binnenkomt. **Automatisch live** (minuten vooraf)
en **automatisch stoppen** (minuten na het eindscherm) staan in de instellingen.

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
4. Zet `ANTHROPIC_API_KEY` (voor het lezen van de website en het typen van onderdelen).
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
YouTube-beelden naar `data/scenes`.
