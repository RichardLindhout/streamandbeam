import type { Service, StreamStatus, View, YouTubeCheck } from "./api";
import { serviceStart } from "./api";

/** The number of TVs in church. */
export const BOARDS = 2;

/** One line in the status light's popover; `warn` lines only make the light amber, `detail`
 *  says what is wrong. */
export type Health = { label: string; ok: boolean; warn?: boolean; detail?: string };

/** The sound input is not there or sends nothing: never normal, also not before the service. */
export function sourceProblem(status: StreamStatus) {
  return status.mode !== "stop" && (status.audioIssue === "niet-gevonden" || status.audioIssue === "geen-invoer");
}

/** The status light: green, amber (the boards, or the sound outside live) or red, with its lines.
 *  `silent`: what is wrong is that no sound comes in, so the light can say "Geen geluid". */
export function health(status: StreamStatus, yt: YouTubeCheck | null, connected: boolean, alarm: boolean) {
  const live = status.mode === "live";
  const lines: Health[] = [];
  if (!connected) lines.push({ label: "Verbinding", ok: false });
  if (live) lines.push({ label: "YouTube", ok: !status.error });
  let silent = false;
  if (status.mode !== "stop") {
    const ytSilent = !!(live && yt?.at && yt.live && !yt.audio);
    // In the preview between services silence is normal: only "let op" then.
    const warn = !ytSilent && !sourceProblem(status) && (!live || status.audioIssue === "niet-usb");
    const ok = !status.audioIssue && !ytSilent;
    lines.push({ label: "Geluid", ok, warn, detail: status.audioWarning || (ytSilent ? "Geen geluid op YouTube" : "") });
    // Sound from the PC itself instead of the USB sound card is not "no sound".
    silent = !ok && status.audioIssue !== "niet-usb";
  }
  const boards = status.boards || 0;
  lines.push({ label: `Borden ${Math.min(boards, 99)} van ${BOARDS}`, ok: boards >= BOARDS, warn: true });
  const bad = !connected || alarm || lines.some((l) => !l.ok && !l.warn);
  const level: "ok" | "warn" | "bad" = bad ? "bad" : lines.some((l) => !l.ok) ? "warn" : "ok";
  return { level, lines, silent };
}

/** Something the volunteer must act on now. At most one is shown, the first in this list. */
export type Alarm = { id: string; title: string; button: string; action: "retry" | "seen" | "begin" | "settings" };

/** Every alarm whose condition holds now, most urgent first. Only while live, except a failed
 *  start and a sound input that is missing (that must be fixed before the service). */
export function alarms(state: View | null, svc: Service | undefined, status: StreamStatus, yt: YouTubeCheck | null, now = Date.now()): Alarm[] {
  const live = status.mode === "live";
  const list: Alarm[] = [];
  if (status.autoStartError && !live) list.push({ id: "start", title: "Uitzending start niet", button: "Opnieuw", action: "retry" });
  if (live && status.error) list.push({ id: "hapert", title: "Uitzending hapert", button: "Gezien", action: "seen" });
  if (sourceProblem(status)) {
    const title = status.audioIssue === "niet-gevonden" ? "Geluidsbron niet aangesloten" : "Geen geluidsinvoer";
    list.push({ id: "geluidsbron", title, button: "Geluidsbron", action: "settings" });
  } else if (live && (status.audioIssue === "stilte" || (yt?.at && yt.live && !yt.audio))) {
    list.push({ id: "geluid", title: "Geen geluid op YouTube", button: "Gezien", action: "seen" });
  }
  if (!live) return list;
  const slide = state?.slides?.[state.index];
  const kind = slide ? svc?.items?.[slide.item]?.kind ?? slide.kind : undefined;
  const start = serviceStart(svc);
  if (kind === "welkom" && start && now >= +start + 8 * 60_000)
    list.push({ id: "begonnen", title: "Dienst al begonnen?", button: "Begin dienst", action: "begin" });
  return list;
}

/** Calm news about the sound for the band under the top bar (no alarm): silence in the preview,
 *  or the sound coming from the PC itself while the USB sound card is there. */
export function soundNotice(status: StreamStatus): string {
  if (status.mode === "stop") return "";
  if (status.audioIssue === "niet-usb") return "Geluid komt niet van de USB-geluidskaart";
  if (status.audioIssue === "stilte" && status.mode !== "live") return `Geen geluid${status.audioDevice ? ` van ${status.audioDevice}` : ""}`;
  return "";
}
