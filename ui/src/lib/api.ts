// Types of the JSON the Rust program sends (camelCase), and small helpers to talk to it.

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
  since: number;
  boards: number;
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
  autoStopMinutes: number;
  youtubeClientId: string;
  youtubeClientSecret: string;
  youtubePrivacy: string;
  youtubeTest: boolean;
  autopilot: string;
  youtubeChannel: string;
};

export const post = (url: string, body?: unknown) =>
  fetch(url, { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(body ?? {}) });

/** Moves the slides: { index }, { delta }, or { kind } ("zwart" toggles black boards). */
export const go = (body: { index?: number; delta?: number; kind?: string }) => post("/api/go", body);

export const saveService = (s: Service) => post("/api/service", s);

export async function quick(text: string): Promise<Item[]> {
  const r = await post("/api/quick", { text });
  if (!r.ok) throw new Error(await r.text());
  return r.json();
}

export function hash(s: string) {
  let h = 0;
  for (let i = 0; i < s.length; i++) h = (h * 31 + s.charCodeAt(i)) | 0;
  return (h >>> 0).toString(36);
}

export function clock(since: number, now = Date.now()) {
  if (!since) return "00:00:00";
  const s = Math.max(0, Math.floor(now / 1000 - since));
  const p = (n: number) => String(n).padStart(2, "0");
  return `${p(Math.floor(s / 3600))}:${p(Math.floor((s % 3600) / 60))}:${p(s % 60)}`;
}

declare global {
  function renderSlide(el: HTMLElement, slide: Slide | null | undefined, black?: boolean, layout?: { portrait?: boolean; rotate?: number }): void;
  function listen(onState: (v: View) => void): void;
}
