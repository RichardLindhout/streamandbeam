import { useEffect, useRef } from "react";
import type { Service, View } from "@/lib/api";
import { go, hash } from "@/lib/api";
import { cn } from "@/lib/utils";

/** Every screen of the service as a thumbnail of the stream; click to show it. */
export function Strip({ state, svc }: { state: View; svc?: Service }) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const el = ref.current?.querySelector<HTMLElement>("[data-now]");
    if (el && ref.current) ref.current.scrollTo({ left: el.offsetLeft - ref.current.clientWidth / 2 + el.clientWidth / 2, behavior: "smooth" });
  }, [state.index, state.current]);
  return (
    <div ref={ref} className="scroll-thin flex snap-x gap-3 overflow-x-auto pt-1 pb-3">
      {state.slides.map((s, i) => {
        const now = i === state.index;
        return (
          <button
            key={i}
            data-now={now || undefined}
            onClick={() => go({ index: i })}
            className={cn(
              "group w-40 shrink-0 snap-start overflow-hidden rounded-lg bg-card text-left transition",
              now ? "ring-[3px] ring-primary" : i === state.index + 1 ? "ring-1 ring-primary/40" : "ring-1 ring-[#2e2822] hover:ring-primary/40",
            )}
          >
            <div className="relative aspect-video bg-[#1a1612]">
              <img loading="lazy" className="size-full object-cover" src={`/api/thumb?index=${i}&k=${hash(state.current + JSON.stringify(s))}`} alt="" />
              <span className="absolute top-1 left-1 rounded bg-black/70 px-1.5 text-[10px] font-bold text-white/80 tabular-nums">{i + 1}</span>
            </div>
            <div className="px-2.5 py-1.5">
              <div className={cn("truncate text-xs font-semibold", now ? "text-foreground" : "text-foreground/80")}>{s.header || "—"}</div>
              <div className="truncate text-[11px] text-muted-foreground">{s.sub || svc?.items?.[s.item]?.label || " "}</div>
            </div>
          </button>
        );
      })}
    </div>
  );
}
