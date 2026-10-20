import { VideoOff } from "lucide-react";
import type { Mode } from "@/lib/api";
import { cn } from "@/lib/utils";

/** What leaves the computer: the MJPEG preview of the stream. */
export function StreamMonitor({ mode, className, big }: { mode: Mode; className?: string; big?: boolean }) {
  const on = mode !== "stop";
  return (
    <div className={cn("mon aspect-video rounded-xl ring-1 ring-white/10", mode === "live" && "ring-2 ring-live shadow-[0_0_40px_-10px_var(--live)]", className)}>
      {on ? (
        <img src="/preview.mjpg" alt="" />
      ) : (
        <div className="absolute inset-0 grid place-items-center bg-[repeating-linear-gradient(135deg,#0b0f14_0_14px,#0d1218_14px_28px)] p-4 text-center">
          <div className="grid justify-items-center gap-2 text-muted-foreground">
            <VideoOff className={big ? "size-10" : "size-7"} strokeWidth={1.5} />
            <div className={cn("font-semibold text-foreground/80", big ? "text-lg" : "text-sm")}>Uitzending staat uit</div>
            <div className="text-xs">Het beeld verschijnt zodra de uitzending start.</div>
          </div>
        </div>
      )}
      <div className="absolute top-2.5 left-2.5 flex gap-1.5">
        {mode === "live" ? (
          <span className="flex items-center gap-1.5 rounded-md bg-live px-2 py-1 text-[11px] font-black tracking-widest text-white">
            <span className="size-1.5 animate-pulse-dot rounded-full bg-white" /> LIVE
          </span>
        ) : (
          <span className="rounded-md bg-black/70 px-2 py-1 text-[11px] font-bold tracking-widest text-white/80 uppercase backdrop-blur">
            {mode === "opnemen" ? "● Opname" : mode === "voorbeeld" ? "Voorbeeld" : "Uitzending"}
          </span>
        )}
      </div>
    </div>
  );
}

/** Sound level of the stream: green, amber near the top, red when it clips. */
export function AudioMeter({ db, off }: { db: number; off: boolean }) {
  const pct = off ? 0 : Math.max(0, Math.min(100, ((db + 60) / 60) * 100));
  const segs = 32;
  const lit = Math.round((pct / 100) * segs);
  return (
    <div className="flex h-2.5 gap-[3px]" title={off ? "Geen geluid (uitzending staat uit)" : `${db.toFixed(0)} dB`}>
      {Array.from({ length: segs }, (_, i) => {
        const color = i >= segs - 2 ? "bg-live" : i >= segs - 7 ? "bg-warn" : "bg-ok";
        return <div key={i} className={cn("flex-1 rounded-[2px] transition-opacity duration-100", i < lit ? color : "bg-white/[0.06]")} />;
      })}
    </div>
  );
}
