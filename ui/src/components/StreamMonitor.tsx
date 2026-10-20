import { useRef } from "react";
import { Check, VideoOff, X } from "lucide-react";
import type { Mode, StreamStatus, YouTubeCheck } from "@/lib/api";
import { cn } from "@/lib/utils";

/** What leaves the computer: the MJPEG preview of the stream. */
export function StreamMonitor({ mode, className, big }: { mode: Mode; className?: string; big?: boolean }) {
  const on = mode !== "stop";
  return (
    <div className={cn("mon aspect-video rounded-xl ring-1 ring-[#3a3229]", mode === "live" && "ring-2 ring-live shadow-[0_0_40px_-10px_var(--live)]", className)}>
      {on ? (
        <img src="/preview.mjpg" alt="" />
      ) : (
        <div className="absolute inset-0 grid place-items-center bg-[repeating-linear-gradient(135deg,#15110e_0_14px,#18140f_14px_28px)] p-4 text-center">
          <div className="grid justify-items-center gap-2 text-muted-foreground">
            <VideoOff className={big ? "size-10" : "size-7"} strokeWidth={1.5} />
            <div className={cn("font-semibold text-foreground/80", big ? "text-lg" : "text-sm")}>Uitzending staat uit</div>
            <div className="text-xs">Het beeld verschijnt zodra de uitzending start.</div>
          </div>
        </div>
      )}
      <div className="absolute top-2.5 left-2.5 flex gap-1.5">
        {mode === "live" ? (
          <span className="flex items-center gap-1.5 rounded-md bg-live px-2 py-1 text-[11px] font-black tracking-widest text-[#fff6ef]">
            <span className="size-1.5 animate-pulse-dot rounded-full bg-white" /> LIVE
          </span>
        ) : (
          <span className="rounded-md bg-black/70 px-2 py-1 text-[11px] font-bold tracking-widest text-[#efe6d6]/85 uppercase backdrop-blur">
            {mode === "opnemen" ? "● Opname" : mode === "voorbeeld" ? "Voorbeeld" : "Uitzending"}
          </span>
        )}
      </div>
    </div>
  );
}

/** How loud the sound going to YouTube is (−60…0 dB), with a peak tick that falls back slowly,
 *  and one line saying whether viewers on YouTube can hear it. */
export function SoundBlock({ status, yt }: { status: StreamStatus; yt: YouTubeCheck | null }) {
  const off = status.mode === "stop";
  const db = off ? -60 : Math.max(-60, Math.min(0, status.audioDb));
  const pct = ((db + 60) / 60) * 100;
  const peak = useRef({ v: 0, at: 0 });
  const now = Date.now();
  if (pct >= peak.current.v || now - peak.current.at > 1500) peak.current = { v: pct, at: now };
  const color = db > -3 ? "var(--live)" : db > -12 ? "var(--warn)" : "var(--ok)";

  let line: React.ReactNode;
  if (status.mode !== "live") line = <span className="text-muted-foreground">Nog niet live</span>;
  else if (!yt?.at) line = <span className="text-muted-foreground">Bezig met controleren…</span>;
  else if (yt.audio)
    line = (
      <span className="text-[#c9d6a6]">
        <Check className="mr-1 inline size-4 -translate-y-px" strokeWidth={3} />
        Op YouTube is geluid te horen <span className="text-muted-foreground">· {Math.round(yt.meanDb)} dB · {ago(yt.at)}</span>
      </span>
    );
  else
    line = (
      <span className="font-semibold text-[#f3a3a8]">
        <X className="mr-1 inline size-4 -translate-y-px" strokeWidth={3} /> Op YouTube is geen geluid te horen
      </span>
    );

  return (
    <div className="grid gap-2.5 rounded-xl border bg-background/40 px-4 py-3.5">
      <div className="flex items-baseline justify-between">
        <span className="hymnal text-[14px] text-[#cbbb9f]">geluid naar youtube</span>
        <span className="w-14 text-right text-sm text-muted-foreground tabular-nums">{off ? "—" : `${Math.round(db)} dB`}</span>
      </div>
      <div className="relative h-3 overflow-hidden rounded-full bg-white/[0.06]">
        <div
          className="absolute inset-y-0 left-0 rounded-full transition-[width,background-color] duration-300 ease-out"
          style={{ width: `${pct}%`, background: `linear-gradient(90deg, color-mix(in srgb, ${color} 55%, transparent), ${color})` }}
        />
        {!off && <div className="absolute inset-y-0 w-[3px] rounded-full bg-[#efe6d6]/80 transition-[left] duration-300" style={{ left: `calc(${peak.current.v}% - 2px)` }} />}
      </div>
      <div className="text-[14px]">{line}</div>
    </div>
  );
}

function ago(at: number) {
  const s = Math.max(0, Math.round(Date.now() / 1000 - at));
  if (s < 60) return `${s} s geleden`;
  return `${Math.round(s / 60)} min geleden`;
}
