// Types of the JSON the Rust program sends (camelCase), and small helpers to talk to it.
import { toast } from "sonner";

export type CustomSlide = { header: string; sub: string; text: string; footer: string };

export type Item = {
  id: string;
  kind: string;
  label: string;
  ref: string;
  text: string;
  problem: string;
  custom: CustomSlide[];
  [k: string]: unknown;
};

export type Service = {
  id: string;
  date: string;
  label: string;
  time: string;
  predikant: string;
  text: Item;
  theme: string;
  points: string[];
  next: string;
  problems: string[];
  youtubeId: string;
  items: Item[];
  /** The operator's own boards; what is absent or null is automatic. */
  boardCustom?: BoardCustom;
  /** Changed by the operator: fetching the website does not replace it. */
  edited?: boolean;
  /** The website has a newer version, waiting in `website`. */
  websiteChanged?: boolean;
  website?: { summary: string; problems: string[] } | null;
  /** From the church's schedules: the special day ("Heilig Avondmaal"), "" on other days. */
  specialDay?: string;
  [k: string]: unknown;
};

/** How many dates a schedule has, from when to when (YYYY-MM-DD). */
export type Span = { aantal: number; van: string; tot: string };
export type ScheduleSummary = { collectes: Span; dagen: Span; voorzang: Span };

/** One date in the coming weeks, from the schedules. */
export type Upcoming = {
  datum: string;
  dag: string;
  collectes: string[];
  voorzangMorgen: string;
  voorzangMiddag: string;
};

/** GET /api/roosters: where the schedules come from (eigen = imported here) and what is coming. */
export type Schedules = { bron: string; ingelezen: string; eigen: boolean; samenvatting: ScheduleSummary; komend: Upcoming[] };

/** What an import of the church's workbook found. */
export type ImportReport = { bron: string; samenvatting: ScheduleSummary; behouden: string[]; waarschuwingen: string[] };

/** GET /api/roosters/dag: what the schedules say about one date and time. */
export type ScheduleDay = { dag: string; voorzang: string; collectes: string[] };

export type BoardTheme = { tekst: string; thema: string; punten: string[] };
export type BoardCustom = { liturgie?: string[] | null; thema?: BoardTheme | null; slotzang?: string | null };

export type Slide = {
  item: number;
  kind: string;
  header: string;
  sub: string;
  lines?: string[];
  verses?: { verse: number; text: string }[];
  theme?: string;
  points?: string[];
  list?: string[];
  /** On the sermon slide and theme board on screen: the point the operator tapped. */
  active?: number | null;
  footer?: string;
};

export type View = {
  services: Service[];
  current: string;
  index: number;
  black: boolean;
  /** During the sermon: the point of the theme on screen (null = the whole theme). */
  point?: number | null;
  slides: Slide[];
  boards: Slide[];
  overrideBoard: Slide | null;
  overrideStream: boolean;
  boardBackground: number;
  /** Unix seconds of the last successful website fetch (0 = never). */
  lastFetched?: number;
  /** Why the last fetch failed ("" = fine). */
  fetchError?: string;
  /** A quiet note about the last fetch: how the AI double check went, or that it is off. */
  fetchNote?: string;
  /** The screens still show the slide as it was before the last change. */
  held?: boolean;
  /** While held: the new version of the board on screen. */
  freshBoard?: Slide | null;
  /** The church's own screens, for the boards and the stream. */
  layouts?: Layout[];
  /** Own screen put on the boards by hand ("" = none). */
  boardLayout?: string;
  /** Own screen put into the stream by hand ("" = none). */
  streamLayout?: string;
};

export type Target = "uitzending" | "bord";
export type BoxKind = "tekst" | "afbeelding" | "vlak" | "camera" | "qr";

/** One box of an own screen, in pixels of the picture (1920x1080 stream, 1080x1920 board). */
export type LayoutBox = {
  id: string;
  kind: BoxKind;
  x: number;
  y: number;
  w: number;
  h: number;
  text: string;
  font: "serif" | "sans";
  size: number;
  bold: boolean;
  color: string;
  align: "links" | "midden" | "rechts";
  valign: "boven" | "midden" | "onder";
  fill: string;
  opacity: number;
  radius: number;
  round: boolean;
  image: string;
  fit: "vullen" | "passend";
};

export type Layout = {
  id: string;
  name: string;
  target: Target;
  /** The moment whose automatic picture it replaces; "" = only by hand. */
  moment: string;
  background: string;
  boxes: LayoutBox[];
};

export type Mode = "stop" | "voorbeeld" | "opnemen" | "live";

/** What is wrong with the sound ("" = fine), most serious first. */
export type AudioIssue = "" | "niet-gevonden" | "geen-invoer" | "stilte" | "niet-usb";

export type StreamStatus = {
  /** The mode chosen; after a failure the program keeps trying to get there (see `error`). */
  mode: Mode;
  /** What is wrong with picture, sound or recording ("" = fine). */
  error: string;
  audioDb: number;
  /** The file being recorded now, in full ("" = not recording). */
  recording: string;
  audioWarning: string;
  audioIssue: AudioIssue;
  /** The sound input the stream records from, e.g. "Microfoon (USB Audio CODEC)". */
  audioDevice: string;
  /** Unix seconds the stream started (0 = not running). */
  since: number;
  boards: number;
  /** A camera is set up (false = test picture). */
  camera?: boolean;
  /** The camera picture is coming in. */
  cameraLive?: boolean;
  /** While live: "geen" (no YouTube set up, only recording), "verbinden", "ok" or "fout" (still recording). */
  youtube?: "" | "geen" | "verbinden" | "ok" | "fout";
  /** Why YouTube does not work. */
  youtubeError?: string;
  /** Unix seconds YouTube is tried again (0 = not waiting). */
  youtubeRetryAt?: number;
  /** Logged in to YouTube or a stream key: going live also sends to YouTube. */
  youtubeConfigured?: boolean;
  /** The folder with the recordings, in full. */
  recordDir?: string;
  /** Unix seconds the stream will go live by itself (0 or absent = not planned). */
  autoStartAt?: number;
  /** The service that auto-start is for. */
  autoStartService?: string;
  /** Why the planned start failed ("" = fine). */
  autoStartError?: string;
  /** Unix seconds the stream will stop by itself after the end card (0 = not planned). */
  autoStopAt?: number;
  /** Unix seconds the end card (aftiteling) started (0 = not showing). */
  endCardAt?: number;
};

export type ListenStatus = {
  active: boolean;
  heard: string;
  suggestion: { slide: number; label: string; reason: string; sure: boolean } | null;
  error: string;
};

export type YouTubeCheck = {
  at: number;
  live: boolean;
  audio: boolean;
  meanDb: number;
  maxDb: number;
  problem: string;
  channel: string;
  streaming: boolean;
};

/** A sound input of the church PC. */
export type SoundInput = { name: string; usb: boolean; default: boolean };

/** audioSource for an input chosen by name. */
export const DEVICE = "apparaat:";

/** Who the church is (see kerk.rs). */
export type Kerk = {
  naam: string;
  website: string;
  liturgieUrl: string;
  kerktijdenId: number;
  fotoStart: string;
  fotoEinde: string;
  logo: string;
};

export type Settings = {
  cameraIp: string;
  cameraRtsp: string;
  /** "" = the default of Windows, "apparaat:<name>", "test" = test tone, or a GStreamer line. */
  audioSource: string;
  videoEncoder: string;
  rtmpUrl: string;
  streamKey: string;
  recordDir: string;
  presets: { overzicht: number; orgel: number; preekstoel: number; doopvont: number };
  qrLink: string;
  introSeconds: number;
  themeSeconds: number;
  autoStartMinutes: number;
  /** Seconds after the end card before the stream stops by itself. */
  autoStopSeconds?: number;
  /** Older programs: minutes after the end. */
  autoStopMinutes?: number;
  /** Bumped by the program on every save; a stale save is refused with 409. */
  settingsVersion?: number;
  youtubeClientId: string;
  youtubeClientSecret: string;
  youtubePrivacy: string;
  youtubeTest: boolean;
  autopilot: string;
  youtubeChannel: string;
  kerk: Kerk;
  /** The boards also show the text of the slotzang verses (off: only which psalm). */
  slotzangTekst: boolean;
  [k: string]: unknown;
};

/** Asks "Je bent live. <question> Kijkers zien dit direct."; set by <LiveConfirm> in the app. */
let askLive: (what: string) => Promise<boolean> = async (what) => confirm(`Je bent live. ${what}`);
export function setLiveConfirm(f: (what: string) => Promise<boolean>) {
  askLive = f;
}

async function send(url: string, body?: unknown) {
  try {
    return await fetch(url, { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(body ?? {}) });
  } catch {
    return new Response("Geen verbinding met de kerk-PC", { status: 503 });
  }
}

/** POSTs JSON. While live, the program first asks to confirm an action viewers would see
 *  (409 with X-Bevestigen): then the volunteer is asked, and on yes it is sent again.
 *  A refusal or a lost connection is always shown as a message, unless `quiet` is set because
 *  the caller shows it itself. Cancelling gives status 499. Never throws. */
export async function post(url: string, body?: unknown, opts?: { quiet?: boolean }): Promise<Response> {
  let r = await send(url, body);
  if (r.status === 409 && r.headers.get("X-Bevestigen")) {
    if (!(await askLive(await r.text()))) return new Response("", { status: 499 });
    r = await send(url + (url.includes("?") ? "&" : "?") + "bevestigd=1", body);
  }
  if (!r.ok && r.status !== 499 && !opts?.quiet) toast.error(await errorText(r), { duration: 8000 });
  return r;
}

/** The text of a refused request, short enough for a message. */
export async function errorText(r: Response) {
  let t = "";
  try { t = (await r.clone().text()).trim(); } catch { /* no body */ }
  if (!t || t.startsWith("<") || t.length > 160) t = r.status === 409 ? "Kan nu niet" : "Lukt niet";
  return t;
}

/** Moves the slides: { index }, { delta }, or { kind } ("zwart" toggles black boards). */
export const go = (body: { index?: number; delta?: number; kind?: string }) => post("/api/go", body);

let lastStep = 0;
/** Volgende / Terug from a button or a key: presses within one second of the last are ignored. */
export function step(delta: 1 | -1) {
  const now = Date.now();
  if (now - lastStep < 1000) return false;
  lastStep = now;
  go({ delta });
  return true;
}

/** During the sermon: a point of the theme (0 = the first) on the boards and, for a moment,
 *  in the stream; null = the whole theme again. */
export const setPoint = (point: number | null) => post("/api/point", { point });

export const setStreamMode = (mode: Mode) => post("/api/stream", { mode });

/** At the end card: wait five more minutes before stopping. */
export const later = () => post("/api/stream/later");

/** Saves a service. The screens keep what they show, unless `show` (asks to confirm while live). */
export const saveService = (s: Service, show?: boolean) => post("/api/service" + (show ? "?tonen=1" : ""), s);

/** Typed parts ("ps 42:5, gebed, joh 3:16") → items. The program reads them itself (no key
 *  needed); what it does not understand comes back with `problem` set. */
export async function quick(text: string): Promise<Item[]> {
  const r = await post("/api/quick", { text }, { quiet: true });
  if (!r.ok) throw new Error(await errorText(r));
  return r.json();
}

export function hash(s: string) {
  let h = 0;
  for (let i = 0; i < s.length; i++) h = (h * 31 + s.charCodeAt(i)) | 0;
  return (h >>> 0).toString(36);
}

/** "dienst-2026-11-08-091500.mp4" of a full path. */
export const fileName = (path: string) => path.split(/[\\/]/).pop() || path;

/** Unix seconds (or milliseconds) → Date; null for 0/absent. */
export function at(t?: number | null) {
  if (!t || !isFinite(t) || t <= 0) return null;
  return new Date(t > 1e12 ? t : t * 1000);
}

const p2 = (n: number) => String(n).padStart(2, "0");
/** "09:15" */
export function hhmm(d: Date | null) {
  return d ? `${p2(d.getHours())}:${p2(d.getMinutes())}` : "";
}

/** When the service starts, from its date and time. */
export function serviceStart(s?: Service) {
  if (!s) return null;
  const d = new Date(`${s.date}T${(s.time || "00:00").slice(0, 5)}:00`);
  return isNaN(+d) ? null : d;
}

const MONTHS = ["januari", "februari", "maart", "april", "mei", "juni", "juli", "augustus", "september", "oktober", "november", "december"];
const SHORT = ["jan", "feb", "mrt", "apr", "mei", "jun", "jul", "aug", "sep", "okt", "nov", "dec"];
const DAYS = ["zondag", "maandag", "dinsdag", "woensdag", "donderdag", "vrijdag", "zaterdag"];
const cap = (s: string) => s[0].toUpperCase() + s.slice(1);

/** "Zondag 25 oktober" */
export function longDate(s: Service) {
  const d = new Date(s.date + "T12:00:00");
  if (isNaN(+d)) return s.date;
  return cap(`${DAYS[d.getDay()]} ${d.getDate()} ${MONTHS[d.getMonth()]}`);
}

/** "Zo 25 okt" */
export function shortDate(s: Service) {
  return shortDay(s.date);
}

/** "2026-10-25" → "Zo 25 okt" */
export function shortDay(date: string) {
  const d = new Date(date + "T12:00:00");
  if (isNaN(+d)) return date;
  return cap(`${DAYS[d.getDay()].slice(0, 2)} ${d.getDate()} ${SHORT[d.getMonth()]}`);
}

/** "2026-10-25" → "25 okt 2026" */
export function dayMonthYear(date: string) {
  const d = new Date(date + "T12:00:00");
  if (isNaN(+d)) return date;
  return `${d.getDate()} ${SHORT[d.getMonth()]} ${d.getFullYear()}`;
}

/** Today as YYYY-MM-DD, in local time. */
export function today() {
  const d = new Date();
  return `${d.getFullYear()}-${p2(d.getMonth() + 1)}-${p2(d.getDate())}`;
}

declare global {
  function renderSlide(el: HTMLElement, slide: Slide | null | undefined, black?: boolean, layout?: { portrait?: boolean; rotate?: number }): void;
  function listen(onState: (v: View) => void): void;
  function renderLayout(el: HTMLElement, layout: Layout, fields: Record<string, string>, opts?: { portrait?: boolean; rotate?: number }): void;
  function slideFields(slide: Slide | null | undefined): Record<string, string>;
}
