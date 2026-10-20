import { useEffect, useRef, useState } from "react";
import type { Item, ListenStatus, Service, StreamStatus, View } from "@/lib/api";
import { at, hhmm, later, serviceStart, setStreamMode, step } from "@/lib/api";
import { SUNG, itemDetail, itemLabel } from "@/lib/kinds";
import { chime } from "@/lib/chime";
import { cn } from "@/lib/utils";
import { Confirm } from "@/components/Confirm";
import { BoardColumn } from "@/components/BoardColumn";
import { OrderEdit } from "@/components/OrderEdit";
import { Pencil, Play } from "lucide-react";

type Props = { state: View | null; svc?: Service; status: StreamStatus; lis: ListenStatus | null };

/** Where the service is: the current part (by item), its slides, and the next part. */
export function position(state: View | null, svc?: Service) {
  const slides = state?.slides || [];
  const index = Math.min(Math.max(0, state?.index ?? 0), Math.max(0, slides.length - 1));
  const slide = slides[index];
  const cur = slide?.item ?? -1;
  const item = svc?.items?.[cur];
  const kind = item?.kind ?? slide?.kind ?? "";
  const mine = slides.map((s, i) => [s, i] as const).filter(([s]) => s.item === cur);
  const nextSlide = slides[index + 1];
  const nextItem = nextSlide ? svc?.items?.[nextSlide.item] : undefined;
  const nxt = slides.slice(index + 1).find((s) => s.item !== cur)?.item;
  return { slides, index, slide, cur, item, kind, mine, nextSlide, nextItem, nxt };
}

/** The second line of the current part: verse, slide, time to the start, or when the stream stops. */
function subline(p: ReturnType<typeof position>, svc: Service | undefined, status: StreamStatus, now: number) {
  if (p.kind === "welkom") {
    const start = serviceStart(svc);
    if (!start) return "";
    const min = Math.ceil((+start - now) / 60_000);
    return min > 0 ? `dienst ${hhmm(start)} · nog ${min} min` : `dienst ${hhmm(start)}`;
  }
  if (p.kind === "einde") {
    const stop = at(status.autoStopAt);
    if (stop && status.mode !== "stop") return `uitzending stopt om ${hhmm(stop)}`;
    if (status.mode === "stop") return "Uitzending gestopt";
    return "";
  }
  if (SUNG.has(p.kind)) {
    // Count the verse slides only (the announcement slide has no lines and is not a verse).
    const verses = p.mine.filter(([s]) => s.lines?.length || s.verses?.length);
    const list = verses.length ? verses : p.mine;
    const n = list.findIndex(([, i]) => i === p.index);
    return list.length > 1 && n >= 0 ? `vers ${n + 1} van ${list.length}` : "";
  }
  const n = p.mine.findIndex(([, i]) => i === p.index);
  return p.mine.length > 1 && n >= 0 ? `dia ${n + 1} van ${p.mine.length}` : "";
}

/** The order of the service: done parts grey, the current one large on parchment, the next marked.
 *  Only to read: tapping does nothing, and it scrolls by itself. */
function Order({ svc, p, sub, hint }: { svc?: Service; p: ReturnType<typeof position>; sub: string; hint?: number }) {
  const box = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const b = box.current, el = b?.querySelector<HTMLElement>("[data-now]");
    if (b && el) b.scrollTo({ top: Math.max(0, el.offsetTop - b.clientHeight / 3 + 8), behavior: "smooth" });
  }, [p.index, p.cur, svc?.items?.length]);

  if (!svc) return <div className="grid flex-1 place-items-center text-[28px] text-muted-foreground">Geen dienst</div>;
  return (
    <div ref={box} className="relative min-h-0 flex-1 cursor-default overflow-hidden px-8 py-4 select-none [mask-image:linear-gradient(to_bottom,transparent,#000_28px,#000_calc(100%-36px),transparent)]">
      <div className="grid gap-1 pb-[60vh]">
        {svc.items.map((it: Item, i: number) => {
          const label = itemLabel(it), detail = itemDetail(it);
          const ring = hint === i && i !== p.cur && "animate-hint rounded-xl";
          if (i === p.cur)
            return (
              <div key={it.id || i} data-now className="my-2 rounded-2xl border-2 border-[#e2cf9f] border-l-[6px] border-l-primary bg-[#fdf8ec] px-6 py-4 shadow-[0_10px_30px_-18px_rgb(90_62_43/0.6)]">
                <div className="flex items-baseline gap-6">
                  <span className="shrink-0 text-[40px] leading-tight font-bold tracking-wide uppercase">▶&nbsp;&nbsp;{label}</span>
                  {detail && <span className="min-w-0 truncate text-[32px] leading-tight">{detail}</span>}
                </div>
                {sub && <div className="mt-1 pl-[52px] text-[28px] leading-tight tabular-nums text-[#5a4632]">{sub}</div>}
              </div>
            );
          if (p.cur >= 0 && i < p.cur)
            return (
              <div key={it.id || i} className={cn("flex items-baseline gap-5 py-1 pl-2 text-[24px] text-done", ring)}>
                <span className="w-7 shrink-0">✓</span>
                <span className="shrink-0">{label}</span>
                {detail && <span className="min-w-0 truncate">{detail}</span>}
              </div>
            );
          if (i === p.nxt)
            return (
              <div key={it.id || i} className={cn("flex items-baseline gap-5 border-l-[3px] border-primary py-1.5 pl-5 text-[30px]", ring)}>
                <span className="hymnal shrink-0 text-[20px] text-[#7a5a1c]">hierna</span>
                <span className="shrink-0 font-semibold">{label}</span>
                {detail && <span className="min-w-0 truncate">{detail}</span>}
              </div>
            );
          return (
            <div key={it.id || i} className={cn("flex items-baseline gap-5 py-1 pl-[2.6rem] text-[26px]", ring)}>
              <span className="shrink-0">{label}</span>
              {detail && <span className="min-w-0 truncate text-[#4a3826]">{detail}</span>}
            </div>
          );
        })}
      </div>
    </div>
  );
}

const big = "flex h-[112px] w-[880px] max-w-full min-w-0 flex-col items-center justify-center rounded-2xl px-8 transition active:scale-[0.99]";

/** Terug, and the one big button that moves the service on; at the end: wait or stop. */
function Bottom({ svc, p, status, now }: { svc?: Service; p: ReturnType<typeof position>; status: StreamStatus; now: number }) {
  const [ask, setAsk] = useState<"" | "early" | "stop">("");
  const start = serviceStart(svc);
  const welkom = p.kind === "welkom";
  const einde = p.kind === "einde";
  const call = welkom && !!start && now >= +start - 60_000;
  const chimed = useRef("");
  useEffect(() => {
    // One soft chime when it is time to begin, once per service.
    if (call && svc && chimed.current !== svc.id) {
      chimed.current = svc.id;
      chime();
    }
  }, [call, svc]);

  // Only when the next press really goes to the end screen (a song after the service may follow the blessing).
  const toEnd = p.nextSlide?.kind === "einde";
  const label = welkom ? "Begin dienst →" : toEnd ? "Aftiteling →" : "Volgende →";
  const next = !welkom && !toEnd && p.nxt !== undefined ? itemLabel(svc?.items?.[p.nxt]) : "";
  const live = status.mode === "live";

  function forward() {
    if (welkom && start && now < +start - 5 * 60_000) return setAsk("early");
    step(1);
  }

  return (
    <footer className="flex h-[136px] shrink-0 items-center gap-6 border-t-2 border-walnut/15 bg-[#efe5cc] px-6">
      <button
        disabled={p.index <= 0 || !p.slides.length}
        onClick={() => step(-1)}
        className="h-24 w-[240px] shrink-0 rounded-2xl border-2 border-input bg-card text-[26px] font-semibold transition hover:bg-accent active:scale-[0.98] disabled:border-[#d8cbb0] disabled:bg-transparent disabled:text-[#a8987f]"
      >
        ← Terug
      </button>
      <div className="flex min-w-0 flex-1 items-center justify-center gap-6">
        {einde ? (
          live && (
            <>
              <button
                onClick={() => later()}
                className="h-[112px] w-[300px] shrink-0 rounded-2xl border-2 border-input bg-card text-[26px] font-semibold transition hover:bg-accent active:scale-[0.98]"
              >
                Nog 5 min
              </button>
              <button
                onClick={() => setAsk("stop")}
                className="h-[112px] w-[340px] shrink-0 rounded-2xl bg-walnut text-[30px] font-bold text-white transition hover:bg-[#6b4a33] active:scale-[0.98]"
              >
                Nu stoppen
              </button>
            </>
          )
        ) : (
          <button
            disabled={!p.slides.length}
            onClick={forward}
            className={cn(
              big,
              "bg-primary text-primary-foreground shadow-[0_10px_30px_-12px_rgb(90_62_43/0.5)] hover:bg-primary-hover disabled:opacity-40",
              call && "animate-call",
            )}
          >
            <span className="text-[44px] leading-none font-bold">{label}</span>
            {next && <span className="mt-2 max-w-full truncate text-[24px] leading-none">{next}</span>}
          </button>
        )}
      </div>
      {/* Balances Terug so the big button sits in the middle. */}
      <div className="hidden w-[240px] shrink-0 min-[1500px]:block" />
      <Confirm open={ask === "early"} onOpenChange={(o) => !o && setAsk("")} title="Dienst begint al?" yes="Ja" onYes={() => step(1)} tone="gold" />
      <Confirm open={ask === "stop"} onOpenChange={(o) => !o && setAsk("")} title="Uitzending stoppen?" yes="Stoppen" no="Nog niet" onYes={() => setStreamMode("stop")} />
    </footer>
  );
}

export function Dienst({ state, svc, status, lis }: Props) {
  const [edit, setEdit] = useState(false);
  const now = Date.now();
  const p = position(state, svc);
  const sub = subline(p, svc, status, now);
  const s = lis?.suggestion?.slide;
  const hint = s !== undefined && s !== null ? state?.slides?.[s]?.item : undefined;
  return (
    <>
      <div className="flex min-h-0 flex-1">
        <div className="flex min-w-0 flex-1 flex-col">
          <div className="flex h-[72px] shrink-0 items-center gap-5 px-5 pt-1">
            <div role="radiogroup" aria-label="Lijst" className="flex shrink-0 rounded-2xl border-2 border-input bg-card p-1">
              {([[false, "Bedienen", <Play key="b" />], [true, "Bewerken", <Pencil key="e" />]] as const).map(([on, label, icon]) => (
                <button
                  key={label}
                  role="radio"
                  aria-checked={edit === on}
                  disabled={on && !svc}
                  onClick={() => setEdit(on)}
                  className={cn(
                    "flex h-14 items-center gap-2 rounded-xl px-6 text-[22px] font-semibold transition [&_svg]:size-6",
                    edit === on ? (on ? "bg-primary text-primary-foreground" : "bg-walnut text-white") : "text-muted-foreground hover:bg-accent",
                  )}
                >
                  {icon} {label}
                </button>
              ))}
            </div>
            {edit && <span className="min-w-0 truncate text-[22px] text-[#7a5a1c]">Bewerken — er verandert niets op het scherm</span>}
          </div>
          {edit && svc ? (
            <div className="flex min-h-0 flex-1 flex-col px-3 pb-3">
              <OrderEdit svc={svc} cur={p.cur} />
            </div>
          ) : (
            <Order svc={svc} p={p} sub={sub} hint={hint} />
          )}
        </div>
        <BoardColumn state={state} svc={svc} edit={edit} />
      </div>
      <Bottom svc={svc} p={p} status={status} now={now} />
    </>
  );
}
