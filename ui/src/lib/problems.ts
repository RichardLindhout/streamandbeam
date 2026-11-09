import type { Service, StreamStatus, View, YouTubeCheck } from "./api";
import { serviceStart } from "./api";

/** The number of TVs in church. */
export const BOARDS = 2;

/** One line in the status light's popover; `warn` lines only make the light amber. */
export type Health = { label: string; ok: boolean; warn?: boolean; detail?: string };

/** The status light: green, amber (boards, live without YouTube, or a problem in the preview) or
 *  red, with its lines. */
export function health(status: StreamStatus, yt: YouTubeCheck | null, connected: boolean, alarm: boolean) {
  const live = status.mode === "live";
  const on = status.mode !== "stop";
  // Recording or live, a problem is serious; in the preview only a warning, so it is noticed
  // before the service (and a camera that is off during the week is no alarm).
  const serious = live || status.mode === "opnemen";
  const lines: Health[] = [];
  if (!connected) lines.push({ label: "Verbinding", ok: false });
  if (on) {
    lines.push({ label: live ? "Uitzending" : serious ? "Opname" : "Voorbeeld", ok: !status.error, warn: !serious, detail: status.error });
    lines.push({
      label: "Camera",
      ok: !!status.camera && status.cameraLive !== false,
      warn: !serious,
      detail: status.camera ? "Geen beeld van de camera. Staat de camera aan?" : "Geen camera ingesteld: testbeeld",
    });
  }
  if (live && status.youtube === "geen") lines.push({ label: "YouTube", ok: false, warn: true, detail: "Geen YouTube ingesteld: alleen opname" });
  else if (live) lines.push({ label: "YouTube", ok: status.youtube !== "fout", detail: status.youtubeError ? `${status.youtubeError}. We nemen wel op.` : "" });
  // Live, no sound is a problem; in the preview it is a warning, so a missing USB sound card
  // is noticed before the service instead of during it.
  if (live) lines.push({ label: "Geluid", ok: !status.audioWarning && !(yt?.at && yt.live && !yt.audio), detail: status.audioWarning });
  else if (on) lines.push({ label: "Geluid", ok: !status.audioWarning, warn: true, detail: status.audioWarning });
  const boards = status.boards || 0;
  lines.push({ label: `Borden ${Math.min(boards, 99)} van ${BOARDS}`, ok: boards >= BOARDS, warn: true });
  const bad = !connected || alarm || lines.some((l) => !l.ok && !l.warn);
  const level: "ok" | "warn" | "bad" = bad ? "bad" : lines.some((l) => !l.ok) ? "warn" : "ok";
  const silent = lines.some((l) => l.label === "Geluid" && !l.ok);
  return { level, lines, silent };
}

/** Something the volunteer must act on now. At most one is shown, the first in this list. */
export type Alarm = { id: string; title: string; button: string; action: "retry" | "seen" | "begin" };

/** Every alarm whose condition holds now, most urgent first. Only while live, except a failed start. */
export function alarms(state: View | null, svc: Service | undefined, status: StreamStatus, yt: YouTubeCheck | null, now = Date.now()): Alarm[] {
  const live = status.mode === "live";
  const list: Alarm[] = [];
  if (status.autoStartError && !live) list.push({ id: "start", title: "Uitzending start niet", button: "Opnieuw", action: "retry" });
  if (!live) return list;
  if (status.error) list.push({ id: "hapert", title: "Uitzending hapert", button: "Gezien", action: "seen" });
  // The recording goes on; YouTube is tried again by itself.
  if (status.youtube === "fout") list.push({ id: "youtube", title: "YouTube werkt niet: we nemen wel op", button: "Gezien", action: "seen" });
  if (status.audioWarning || (yt?.at && yt.live && !yt.audio))
    list.push({ id: "geluid", title: status.youtube === "geen" ? "Geen geluid" : "Geen geluid op YouTube", button: "Gezien", action: "seen" });
  const slide = state?.slides?.[state.index];
  const kind = slide ? svc?.items?.[slide.item]?.kind ?? slide.kind : undefined;
  const start = serviceStart(svc);
  if (kind === "welkom" && start && now >= +start + 8 * 60_000)
    list.push({ id: "begonnen", title: "Dienst al begonnen?", button: "Begin dienst", action: "begin" });
  return list;
}
