import { useEffect, useRef } from "react";
import { ChevronRight, CircleAlert, ListOrdered, Tv, Video } from "lucide-react";
import type { ListenStatus, Service, StreamStatus, View, YouTubeCheck } from "@/lib/api";
import { go, post } from "@/lib/api";
import { KindIcon } from "@/lib/kinds";
import { cn } from "@/lib/utils";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { BoardTiles } from "@/components/BoardTiles";
import { SoundBlock, StreamMonitor } from "@/components/StreamMonitor";
import { LiveButton } from "@/components/LiveButton";

type Props = { state: View | null; svc?: Service; status: StreamStatus; lis: ListenStatus | null; yt: YouTubeCheck | null };

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
    <Card className="flex h-full min-h-0 flex-col overflow-hidden">
      <CardHeader><ListOrdered /> Volgorde</CardHeader>
      <div ref={ref} className="scroll-thin relative min-h-0 flex-1 overflow-y-auto p-2">
        {!svc ? (
          <div className="p-4 text-muted-foreground">Nog geen dienst gekozen.</div>
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
                  "relative flex w-full items-center gap-3 rounded-xl px-3 py-2.5 text-left transition-colors duration-300",
                  now ? "bg-primary text-primary-foreground shadow-[0_8px_24px_-12px_var(--primary)]" : "hover:bg-accent",
                  next && "bg-primary/[0.06] ring-1 ring-primary/30 ring-inset",
                )}
              >
                <span className={cn("grid size-8 shrink-0 place-items-center rounded-lg", now ? "bg-black/15" : "bg-secondary text-muted-foreground")}>
                  <KindIcon kind={it.kind} className="size-4" />
                </span>
                <span className={cn("min-w-0 flex-1 leading-tight", past && "opacity-45")}>
                  <span className="block truncate text-[16px] font-semibold">{it.label}</span>
                  {it.ref && <span className={cn("block truncate text-[14px]", now ? "text-primary-foreground/75" : "text-muted-foreground")}>{it.ref}</span>}
                </span>
                {now && <span className="hymnal text-[13px]">nu</span>}
                {next && <span className="hymnal text-[13px] text-primary/80">hierna</span>}
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
  const sub = [...new Set([slide?.sub, label !== slide?.header ? label : ""].filter(Boolean))].join(" · ");
  return (
    <div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_minmax(0,1.3fr)]">
      <div className="flex min-w-0 flex-col justify-center rounded-2xl border bg-card px-6 py-5">
        <div className="mb-1 flex items-center gap-2 text-primary">
          <span className="size-1.5 rounded-full bg-primary" /> <span className="hymnal text-[15px]">nu</span>
        </div>
        <div className="truncate font-serif text-[30px] leading-tight font-semibold tracking-tight">{slide?.header || label || "—"}</div>
        <div className="truncate text-[16px] text-muted-foreground">{sub || " "}</div>
      </div>
      <div className="grid gap-1.5">
        <button
          onClick={() => (suggestion ? post("/api/listen/accept") : go({ delta: 1 }))}
          className={cn(
            "flex min-h-[116px] min-w-0 items-center gap-4 rounded-2xl px-7 py-5 text-left transition hover:brightness-110 active:scale-[0.99]",
            suggestion
              ? "animate-glow border border-primary/60 bg-gradient-to-b from-[#8e2a3b] to-[#661c2a] text-[#f6ead2]"
              : "bg-gradient-to-b from-[#d9b56c] to-[#b48a3e] text-primary-foreground shadow-[0_18px_40px_-16px_rgb(201_164_92/0.75),inset_0_1px_0_rgb(255_255_255/0.35)]",
          )}
        >
          <span className="min-w-0 flex-1">
            <span className={cn("hymnal block text-[15px]", suggestion ? "text-[#e9cf98]" : "text-primary-foreground/70")}>
              {suggestion ? "volgens het gehoorde" : "hierna"}
            </span>
            <span className="block font-serif text-[36px] leading-none font-semibold tracking-tight">{suggestion ? "Ga naar" : "Volgende"}</span>
            <span className={cn("mt-1.5 block truncate text-[16px]", suggestion ? "text-[#f6ead2]/85" : "text-primary-foreground/80")}>
              {suggestion ? suggestion.label : next ? `${next.header}${next.sub ? " · " + next.sub : ""}` : "einde van de dienst"}
            </span>
          </span>
          <ChevronRight className="size-11 shrink-0" strokeWidth={2.5} />
        </button>
        <button onClick={() => go({ delta: -1 })} className="justify-self-start px-2 py-1.5 text-[15px] text-muted-foreground transition hover:text-foreground">
          ← terug
        </button>
      </div>
    </div>
  );
}

export function Dienst({ state, svc, status, lis, yt }: Props) {
  return (
    <main className="grid flex-1 content-start items-start gap-5 p-5 lg:grid-cols-[280px_minmax(0,1fr)] xl:grid-cols-[300px_minmax(0,1fr)_380px]">
      <div className="lg:sticky lg:top-5 lg:row-span-2 lg:h-[calc(100vh-7.5rem)]">
        <Order state={state} svc={svc} />
      </div>

      <NowAndGo state={state} svc={svc} lis={lis} />

      <div className="grid min-w-0 gap-5 lg:max-xl:grid-cols-[minmax(0,1.35fr)_minmax(0,1fr)] xl:contents">
        <Card className="min-w-0">
          <CardHeader><Tv /> Borden in de kerk</CardHeader>
          <CardContent className="mx-auto w-full max-w-[860px] px-5 pt-7 pb-5 xl:px-6">
            <BoardTiles state={state} />
          </CardContent>
        </Card>

        <Card className="min-w-0 xl:col-start-3 xl:row-span-2 xl:row-start-1">
          <CardHeader><Video /> Uitzending</CardHeader>
          <CardContent className="grid gap-4">
            <StreamMonitor mode={status.mode} />
            <SoundBlock status={status} yt={yt} />
            <LiveButton status={status} />
          </CardContent>
        </Card>
      </div>
    </main>
  );
}
