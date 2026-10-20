import { useEffect, useRef } from "react";
import { ChevronLeft, ChevronRight, CircleAlert, Ear, ListOrdered, Tv, Video } from "lucide-react";
import type { ListenStatus, Service, StreamStatus, View } from "@/lib/api";
import { go, post } from "@/lib/api";
import { KindIcon } from "@/lib/kinds";
import { cn } from "@/lib/utils";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { Board } from "@/components/Board";
import { BoardTiles } from "@/components/BoardTiles";
import { AudioMeter, StreamMonitor } from "@/components/StreamMonitor";
import { LiveButton } from "@/components/LiveButton";

type Props = { state: View | null; svc?: Service; status: StreamStatus; lis: ListenStatus | null; onBig: (w: "bord" | "stream") => void };

/** Current and next part of the service (by item), as in the order list. */
export function nowNext(state: View | null) {
  const cur = state?.slides?.[state.index]?.item;
  const nxt = state?.slides?.slice(state.index + 1).find((s) => s.item !== cur)?.item;
  return { cur, nxt };
}

function Order({ state, svc }: { state: View | null; svc?: Service }) {
  const ref = useRef<HTMLDivElement>(null);
  const { cur, nxt } = nowNext(state);
  useEffect(() => {
    // Scroll only the list itself, never the page.
    const box = ref.current, el = box?.querySelector<HTMLElement>("[data-now]");
    if (box && el && (el.offsetTop < box.scrollTop || el.offsetTop + el.offsetHeight > box.scrollTop + box.clientHeight))
      box.scrollTo({ top: el.offsetTop - box.clientHeight / 3, behavior: "smooth" });
  }, [cur]);
  return (
    <Card className="flex min-h-0 flex-col overflow-hidden">
      <CardHeader action={<span className="text-xs text-muted-foreground">tik om te tonen</span>}>
        <ListOrdered /> Volgorde
      </CardHeader>
      <div ref={ref} className="scroll-thin relative min-h-0 flex-1 overflow-y-auto p-2">
        {!svc ? (
          <div className="p-4 text-sm text-muted-foreground">Nog geen dienst. Kies of maak er een bij “Voorbereiden”.</div>
        ) : (
          svc.items.map((it, i) => {
            const now = i === cur, next = i === nxt;
            const past = cur !== undefined && i < cur;
            return (
              <button
                key={it.id || i}
                data-now={now || undefined}
                onClick={() => {
                  const slide = (state?.slides || []).findIndex((s) => s.item === i);
                  if (slide >= 0) go({ index: slide });
                }}
                className={cn(
                  "relative flex w-full items-center gap-3 rounded-xl px-3 py-2.5 text-left transition",
                  now ? "bg-primary text-white shadow-[0_8px_24px_-12px_var(--primary)]" : "hover:bg-accent",
                  next && "bg-white/[0.04] ring-1 ring-white/15 ring-inset",
                )}
              >
                <span className={cn("grid size-8 shrink-0 place-items-center rounded-lg", now ? "bg-white/20" : "bg-secondary text-muted-foreground")}>
                  <KindIcon kind={it.kind} className="size-4" />
                </span>
                <span className={cn("min-w-0 flex-1 leading-tight", past && "opacity-45")}>
                  <span className="block truncate text-[15px] font-semibold">{it.label}</span>
                  {it.ref && <span className={cn("block truncate text-[13px]", now ? "text-white/80" : "text-muted-foreground")}>{it.ref}</span>}
                </span>
                {now && <span className="text-[10px] font-black tracking-widest">NU</span>}
                {next && <span className="text-[10px] font-bold tracking-widest text-muted-foreground">HIERNA</span>}
                {it.problem && <CircleAlert className="size-4 shrink-0 text-warn" />}
              </button>
            );
          })
        )}
      </div>
    </Card>
  );
}

function NowAndGo({ state, svc, lis }: { state: View | null; svc?: Service; lis: ListenStatus | null }) {
  const slide = state?.slides?.[state.index];
  const next = state?.slides?.[(state?.index ?? 0) + 1];
  const suggestion = lis?.suggestion;
  const label = slide ? svc?.items?.[slide.item]?.label : undefined;
  return (
    <div className="grid gap-3 lg:grid-cols-[minmax(0,1fr)_minmax(0,1.35fr)]">
      <div className="flex min-w-0 items-center gap-4 rounded-2xl border bg-card px-5 py-4">
        <div className="min-w-0 flex-1">
          <div className="mb-1 flex items-center gap-2 text-[11px] font-black tracking-[0.16em] text-primary">
            <span className="size-1.5 rounded-full bg-primary" /> NU
            {state && <span className="font-semibold tracking-normal text-muted-foreground">· scherm {state.index + 1} van {state.slides.length}</span>}
          </div>
          <div className="truncate text-[26px] leading-tight font-bold tracking-tight">{slide?.header || label || "—"}</div>
          <div className="truncate text-[15px] text-muted-foreground">{[...new Set([slide?.sub, label !== slide?.header ? label : ""].filter(Boolean))].join(" · ") || " "}</div>
        </div>
      </div>
      <div className="grid grid-cols-[104px_minmax(0,1fr)] gap-3">
        <button
          onClick={() => go({ delta: -1 })}
          className="flex flex-col items-center justify-center gap-1 rounded-2xl border bg-secondary text-sm font-semibold text-muted-foreground transition hover:bg-accent hover:text-foreground active:scale-[0.98]"
        >
          <ChevronLeft className="size-6" />
          Vorige
        </button>
        <button
          onClick={() => (suggestion ? post("/api/listen/accept") : go({ delta: 1 }))}
          className={cn(
            "flex min-w-0 items-center gap-4 rounded-2xl px-6 py-4 text-left text-white transition hover:brightness-110 active:scale-[0.99]",
            suggestion
              ? "animate-glow bg-gradient-to-b from-[#ffbf3c] to-[#e08e00] text-black"
              : "bg-gradient-to-b from-[#3b95ff] to-[#1663e0] shadow-[0_18px_40px_-14px_rgb(47_140_255/0.9),inset_0_1px_0_rgb(255_255_255/0.25)]",
          )}
        >
          <span className="min-w-0 flex-1">
            <span className={cn("block text-[11px] font-black tracking-[0.16em]", suggestion ? "text-black/70" : "text-white/70")}>
              {suggestion ? "VOLGENS HET GEHOORDE" : "HIERNA"}
            </span>
            <span className="block text-[28px] leading-none font-black tracking-tight">{suggestion ? "Ga naar" : "Volgende"}</span>
            <span className={cn("mt-1 block truncate text-[14px]", suggestion ? "text-black/80" : "text-white/85")}>
              {suggestion ? `${suggestion.label} — “${suggestion.reason}”` : next ? `${next.header}${next.sub ? " · " + next.sub : ""}` : "einde van de dienst"}
            </span>
          </span>
          <ChevronRight className="size-10 shrink-0" strokeWidth={2.5} />
        </button>
      </div>
    </div>
  );
}

export function Dienst({ state, svc, status, lis, onBig }: Props) {
  return (
    <main className="grid flex-1 content-start items-start gap-5 p-5 lg:grid-cols-[270px_minmax(0,1fr)] xl:grid-cols-[300px_minmax(0,1fr)_380px]">
      <div className="lg:sticky lg:top-5 lg:row-span-3 lg:h-[calc(100vh-7.5rem)]">
        <Order state={state} svc={svc} />
      </div>

      <div className="grid min-w-0 gap-3">
        <NowAndGo state={state} svc={svc} lis={lis} />
        {lis?.heard && (
          <div className="flex items-center gap-2 px-1 text-sm text-muted-foreground italic"><Ear className="size-4 not-italic" /> Gehoord: {lis.heard}</div>
        )}
      </div>

      <div className="grid min-w-0 gap-5 lg:max-xl:grid-cols-[minmax(0,1.35fr)_minmax(0,1fr)] xl:contents">
      <Card className="min-w-0">
        <CardHeader action={<button onClick={() => onBig("bord")} className="text-xs text-muted-foreground hover:text-foreground">groot bekijken</button>}>
          <Tv /> Borden in de kerk
        </CardHeader>
        <CardContent className="mx-auto w-full max-w-[860px] px-5 pt-7 pb-5 xl:px-6">
          <BoardTiles state={state} />
        </CardContent>
      </Card>

      <Card className="min-w-0 xl:col-start-3 xl:row-span-2 xl:row-start-1">
        <CardHeader><Video /> Uitzending</CardHeader>
        <CardContent className="grid gap-5">
          <div className="grid content-start gap-3">
            <button className="block w-full" onClick={() => onBig("stream")} title="Groot bekijken">
              <StreamMonitor mode={status.mode} />
            </button>
            <AudioMeter db={status.audioDb} off={status.mode === "stop"} />
          </div>
          <div className="grid content-start gap-3">
            <LiveButton status={status} />
            {(status.mode === "voorbeeld" || status.mode === "opnemen") && (
              <div className="text-[13px] text-muted-foreground">
                Nu: {status.mode === "voorbeeld" ? "alleen voorbeeld" : "alleen opname"} (niet op YouTube).
              </div>
            )}
            {status.error && <div className="rounded-lg border border-warn/40 bg-warn/10 p-3 text-[13px] text-warn">{status.error}</div>}
          </div>
        </CardContent>
      </Card>
      </div>
    </main>
  );
}

/** Full-screen view of a monitor. */
export function BigMonitor({ which, state, status, close }: { which: "bord" | "stream"; state: View | null; status: StreamStatus; close: () => void }) {
  const slide = state && (state.overrideBoard || (state.boards?.length ? state.boards : state.slides)?.[state.index]);
  return (
    <div className="fixed inset-0 z-50 grid place-items-center bg-black/90 p-6 backdrop-blur" onClick={close}>
      {which === "bord" ? (
        <div className="h-[88vh]"><Board slide={slide} black={state?.black} className="h-full rounded-xl ring-1 ring-white/15" /></div>
      ) : (
        <div className="w-[min(1400px,94vw)]"><StreamMonitor mode={status.mode} big /></div>
      )}
      <div className="fixed bottom-5 text-sm text-muted-foreground">Tik ergens om te sluiten</div>
    </div>
  );
}
