// The church's own screens: what a new box looks like, the moments a screen can replace,
// and the starting points for a new screen. The stream ones follow the church's OBS scenes
// (text on the left with the camera on the right, a line at the bottom, start and end screen).
import type { BoxKind, Layout, LayoutBox, Target } from "./api";

export const SIZE: Record<Target, { w: number; h: number }> = {
  uitzending: { w: 1920, h: 1080 },
  bord: { w: 1080, h: 1920 },
};

export const MOMENTS: Record<Target, { id: string; label: string }[]> = {
  uitzending: [
    { id: "", label: "Alleen met de hand" },
    { id: "welkom", label: "Startbeeld (vóór de dienst)" },
    { id: "aankondiging", label: "Psalm aangekondigd (voorspel)" },
    { id: "psalm", label: "Psalmvers of lied" },
    { id: "lezing", label: "Schriftlezing" },
    { id: "catechismus", label: "Catechismus" },
    { id: "collecte", label: "Collecte" },
    { id: "gebed", label: "Gebed" },
    { id: "thema", label: "Thema van de preek" },
    { id: "camera", label: "Alleen camera (preek, zegen)" },
    { id: "tekst", label: "Overige tekst (formulier, mededeling)" },
    { id: "balk", label: "Overige balk (onderdeel zonder tekst)" },
    { id: "einde", label: "Eindbeeld (na de dienst)" },
  ],
  bord: [
    { id: "", label: "Alleen met de hand" },
    { id: "bord", label: "Liturgiebord" },
    { id: "thema", label: "Thema (tijdens de preek)" },
    { id: "slotzang", label: "Slotzang" },
    { id: "vrij", label: "Eigen tekst" },
  ],
};

export const FIELDS = [
  { id: "kop", label: "Kop" },
  { id: "onder", label: "Onderkop" },
  { id: "tekst", label: "Tekst" },
  { id: "voet", label: "Voetregel" },
  { id: "datum", label: "Datum" },
  { id: "dienst", label: "Dienst" },
  { id: "tijd", label: "Tijd" },
  { id: "predikant", label: "Predikant" },
];

export const KIND_LABEL: Record<BoxKind, string> = { tekst: "Tekst", afbeelding: "Afbeelding", vlak: "Vlak", camera: "Camera", qr: "QR-code" };

const INK = "#44546a"; // the text colour of the church's OBS scenes
const TEAL = "#005e70";

let n = 0;
const id = () => `vak-${Date.now().toString(36)}-${n++}`;

export function box(kind: BoxKind, at: Partial<LayoutBox> = {}): LayoutBox {
  return {
    id: id(), kind, x: 100, y: 100, w: 600, h: 200, text: kind === "tekst" ? "Tekst" : "", font: "sans", size: 60, bold: false,
    color: "#ffffff", align: "links", valign: "boven", fill: kind === "vlak" ? TEAL : "", opacity: 1, radius: 0, round: false, image: "", fit: "vullen",
    ...at,
  };
}

/** Example texts for the editor, per moment. */
export function sample(target: Target, moment: string): Record<string, string> {
  const base = { datum: "Zondag 15 november 2026", dienst: "Middagdienst", tijd: "16:00", predikant: "ds. A. den Hartog", voet: "" };
  if (target === "bord") {
    if (moment === "thema") return { ...base, kop: "De Goede Herder", onder: "Joh.10:11", tekst: "1. Zijn zorg\n2. Zijn offer\n3. Zijn kudde" };
    if (moment === "slotzang") return { ...base, kop: "Slotzang", onder: "Ps 134:3", tekst: "" };
    return { ...base, kop: "Liturgie", onder: "", tekst: "Ps 100:1,2\nPs 119:3\nJoh.10:1-18\nPs 23:1,3\nPs 134:3" };
  }
  switch (moment) {
    case "welkom": return { ...base, kop: base.datum, onder: base.dienst, tekst: "" };
    case "einde": return { ...base, kop: "Vanavond is er een dienst om 18.30 uur.", onder: "", tekst: "" };
    case "aankondiging": case "balk": return { ...base, kop: "Psalm 100 : 1", onder: "Zingen", tekst: "" };
    case "gebed": return { ...base, kop: "Gebed", onder: "", tekst: "" };
    case "camera": return { ...base, kop: "", onder: "", tekst: "" };
    case "lezing":
      return { ...base, kop: "Johannes 10:11-13", onder: "Schriftlezing", tekst: "11 Ik ben de goede Herder; de goede herder stelt zijn leven voor de schapen.\n12 Maar de huurling, en die geen herder is, wien de schapen niet eigen zijn, ziet den wolf komen, en verlaat de schapen, en vliedt.\n13 En de huurling vliedt, overmits hij een huurling is, en heeft geen zorg voor de schapen." };
    case "catechismus":
      return { ...base, kop: "Vraag 57", onder: "Heidelbergse Catechismus", voet: "Zondag 22", tekst: "Wat troost geeft u de opstanding des vleses?\nDat niet alleen mijn ziel na dit leven van stonden aan tot Christus, haar Hoofd, zal opgenomen worden, maar dat ook dit mijn vlees, door de kracht van Christus opgewekt zijnde, wederom met mijn ziel verenigd zal worden." };
    case "collecte": return { ...base, kop: "Collecte", onder: "", tekst: "1. Stichting Kimon\n2. Kerkvoogdij\n3. Aflossing bouwfonds" };
    case "thema": return { ...base, kop: "De Goede Herder", onder: "Thema", tekst: "1. Zijn zorg\n2. Zijn offer\n3. Zijn kudde" };
    default:
      return { ...base, kop: "Psalm 100 : 1", onder: "Zingen", tekst: "Juich, aarde, juicht alom den HEER;\nDient God met blijdschap, geeft Hem eer;\nKomt, nadert voor Zijn aangezicht;\nZingt Hem een vrolijk lofgedicht." };
  }
}

type Start = { label: string; make: () => Omit<Layout, "id"> };

export const STARTS: Record<Target, Start[]> = {
  uitzending: [
    {
      label: "Tekst links, camera rechts",
      make: () => ({
        name: "Tekst links, camera rechts", target: "uitzending", moment: "", background: "#fbfaf7",
        boxes: [
          box("camera", { x: 746, y: 0, w: 1174, h: 1080 }),
          box("tekst", { x: 46, y: 52, w: 660, h: 96, text: "{kop}", bold: true, size: 67, color: INK }),
          box("tekst", { x: 46, y: 160, w: 660, h: 800, text: "{tekst}", font: "serif", size: 49, color: INK }),
          box("tekst", { x: 46, y: 980, w: 660, h: 60, text: "{voet}", bold: true, size: 36, color: INK, valign: "onder" }),
        ],
      }),
    },
    {
      label: "Camera met regel onderaan",
      make: () => ({
        name: "Regel onderaan", target: "uitzending", moment: "", background: "#000000",
        boxes: [
          box("camera", { x: 0, y: 0, w: 1920, h: 1080 }),
          box("vlak", { x: 0, y: 972, w: 1920, h: 108, fill: TEAL, opacity: 0.9 }),
          box("tekst", { x: 60, y: 980, w: 1800, h: 92, text: "{kop}", bold: true, size: 65, align: "midden", valign: "midden" }),
        ],
      }),
    },
    {
      label: "Startbeeld met datum en dienst",
      make: () => ({
        name: "Startbeeld", target: "uitzending", moment: "", background: TEAL,
        boxes: [
          box("afbeelding", { x: 0, y: 0, w: 1920, h: 984, image: "buiten.jpg" }),
          box("tekst", { x: 175, y: 984, w: 1100, h: 96, text: "{datum}", bold: true, size: 74, valign: "midden" }),
          box("tekst", { x: 1290, y: 984, w: 600, h: 96, text: "{dienst}", bold: true, size: 74, align: "rechts", valign: "midden" }),
          box("qr", { x: 27, y: 939, w: 120, h: 120 }),
        ],
      }),
    },
    {
      label: "Eindbeeld",
      make: () => ({
        name: "Eindbeeld", target: "uitzending", moment: "", background: TEAL,
        boxes: [
          box("afbeelding", { x: 0, y: 0, w: 1920, h: 984, image: "interieur.jpg" }),
          box("tekst", { x: 175, y: 984, w: 1715, h: 96, text: "{kop}", bold: true, size: 70, valign: "midden" }),
          box("qr", { x: 27, y: 939, w: 120, h: 120 }),
        ],
      }),
    },
    { label: "Alleen camera", make: () => ({ name: "Alleen camera", target: "uitzending", moment: "", background: "#000000", boxes: [box("camera", { x: 0, y: 0, w: 1920, h: 1080 })] }) },
    { label: "Leeg", make: () => ({ name: "Nieuw beeld", target: "uitzending", moment: "", background: "#000000", boxes: [] }) },
  ],
  bord: [
    {
      // As the church's screens were: plain black with the church in the bottom right corner.
      label: "Zwart met de kerk rechtsonder",
      make: () => ({
        name: "Zwart met kerk", target: "bord", moment: "", background: "#000000",
        boxes: [
          box("tekst", { x: 70, y: 110, w: 940, h: 180, text: "{kop}", bold: true, size: 110, valign: "onder" }),
          box("tekst", { x: 70, y: 340, w: 940, h: 1180, text: "{tekst}", bold: true, size: 96 }),
          box("afbeelding", { x: 690, y: 1600, w: 330, h: 250, image: "logo-wit.png", fit: "passend" }),
        ],
      }),
    },
    {
      label: "Kop met tekst",
      make: () => ({
        name: "Kop met tekst", target: "bord", moment: "", background: "",
        boxes: [
          box("tekst", { x: 70, y: 110, w: 940, h: 180, text: "{kop}", bold: true, size: 110, valign: "onder" }),
          box("vlak", { x: 70, y: 310, w: 940, h: 5, fill: "#ffffff", opacity: 0.4 }),
          box("tekst", { x: 70, y: 370, w: 940, h: 1440, text: "{tekst}", bold: true, size: 96 }),
        ],
      }),
    },
    {
      label: "Groot in het midden",
      make: () => ({
        name: "Groot in het midden", target: "bord", moment: "", background: "",
        boxes: [
          box("tekst", { x: 70, y: 560, w: 940, h: 150, text: "{kop}", size: 80, align: "midden", valign: "onder" }),
          box("tekst", { x: 70, y: 740, w: 940, h: 440, text: "{onder}", bold: true, size: 190, align: "midden" }),
        ],
      }),
    },
    { label: "Leeg (zwart)", make: () => ({ name: "Nieuw bord", target: "bord", moment: "", background: "#000000", boxes: [] }) },
  ],
};
