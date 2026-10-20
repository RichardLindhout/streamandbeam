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
};

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
  active?: number | null;
  footer?: string;
};

export type View = {
  services: Service[];
  current: string;
  index: number;
  black: boolean;
  slides: Slide[];
  boards: Slide[];
  overrideBoard: Slide | null;
  overrideStream: boolean;
  boardBackground: number;
};

export type Mode = "stop" | "voorbeeld" | "opnemen" | "live";

export type StreamStatus = {
  mode: Mode;
  error: string;
  audioDb: number;
  recording: string;
  audioWarning: string;
  /** Unix seconds the stream started (0 = not running). */
  since: number;
  boards: number;
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

export type Settings = {
  cameraIp: string;
  cameraRtsp: string;
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
  [k: string]: unknown;
};

/** POSTs JSON. A refusal (409 and the like) or a lost connection is always shown as a message,
 *  unless `quiet` is set because the caller shows it itself. Never throws. */
export async function post(url: string, body?: unknown, opts?: { quiet?: boolean }): Promise<Response> {
  let r: Response;
  try {
    r = await fetch(url, { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(body ?? {}) });
  } catch {
    r = new Response("Geen verbinding met de kerk-PC", { status: 503 });
  }
  if (!r.ok && !opts?.quiet) toast.error(await errorText(r), { duration: 8000 });
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

export const setStreamMode = (mode: Mode) => post("/api/stream", { mode });

/** At the end card: wait five more minutes before stopping. */
export const later = () => post("/api/stream/later");

export const saveService = (s: Service) => post("/api/service", s);

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
  const d = new Date(s.date + "T12:00:00");
  if (isNaN(+d)) return s.date;
  return cap(`${DAYS[d.getDay()].slice(0, 2)} ${d.getDate()} ${SHORT[d.getMonth()]}`);
}

declare global {
  function renderSlide(el: HTMLElement, slide: Slide | null | undefined, black?: boolean, layout?: { portrait?: boolean; rotate?: number }): void;
  function listen(onState: (v: View) => void): void;
}
