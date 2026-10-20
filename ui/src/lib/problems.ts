import type { Service, StreamStatus, View, YouTubeCheck } from "./api";
import { serviceStart } from "./api";

/** The number of TVs in church. */
export const BOARDS = 2;

/** One line in the status light's popover. */
export type Health = { label: string; ok: boolean };

/** The status light: green, amber (only the boards) or red, with its lines. */
export function health(status: StreamStatus, yt: YouTubeCheck | null, connected: boolean, alarm: boolean) {
  const live = status.mode === "live";
  const running = status.mode !== "stop";
  const lines: Health[] = [];
  if (!connected) lines.push({ label: "Verbinding", ok: false });
  if (live) lines.push({ label: "YouTube", ok: !status.error });
  if (running) lines.push({ label: "Geluid", ok: !status.audioWarning && !(live && yt?.at && yt.live && !yt.audio) });
  const boards = status.boards || 0;
  lines.push({ label: `Borden ${Math.min(boards, 99)} van ${BOARDS}`, ok: boards >= BOARDS });
  const bad = !connected || alarm || lines.some((l) => !l.ok && !l.label.startsWith("Borden"));
  const level: "ok" | "warn" | "bad" = bad ? "bad" : lines.some((l) => !l.ok) ? "warn" : "ok";
  return { level, lines };
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
  if (status.audioWarning || (yt?.at && yt.live && !yt.audio)) list.push({ id: "geluid", title: "Geen geluid op YouTube", button: "Gezien", action: "seen" });
  const slide = state?.slides?.[state.index];
  const kind = slide ? svc?.items?.[slide.item]?.kind ?? slide.kind : undefined;
  const start = serviceStart(svc);
  if (kind === "welkom" && start && now >= +start + 8 * 60_000)
    list.push({ id: "begonnen", title: "Dienst al begonnen?", button: "Begin dienst", action: "begin" });
  return list;
}
