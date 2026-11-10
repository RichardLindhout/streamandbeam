import { useEffect, useRef, useState } from "react";
import type { Item, ListenStatus, Service, Slide, StreamStatus, View } from "@/lib/api";
import { at, go, hhmm, later, serviceStart, setPoint, setStreamMode, step } from "@/lib/api";
import { SUNG, itemDetail, itemLabel } from "@/lib/kinds";
import { chime } from "@/lib/chime";
import { cn } from "@/lib/utils";
import { Confirm } from "@/components/Confirm";
import { BoardColumn } from "@/components/BoardColumn";
import { OrderEdit } from "@/components/OrderEdit";
import { GotoSheet } from "@/components/Header";
import { usePhone } from "@/lib/phone";
import { Eye, EyeOff, LayoutGrid, ListChecks, ListOrdered, Pencil } from "lucide-react";

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
    if (min > 120) {
      const day = start.toLocaleDateString("nl-NL", { weekday: "long", day: "numeric", month: "long" });
      return `dienst ${day} ${hhmm(start)}`;
    }
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

const pointButton = "flex min-h-[72px] w-full items-center gap-4 rounded-xl border-2 px-5 py-2 text-left leading-tight transition active:scale-[0.99]";

/** The theme and its points during the sermon: the service's own, or when it has no points,
 *  those on the theme board (as Show::sermon_theme in the program). */
function sermonTheme(slide: Slide, board?: Slide) {
  const own = slide.points || [];
  if (!own.length && board?.kind === "thema" && board.points?.length) return { theme: board.theme || slide.theme || "", points: board.points };
  return { theme: slide.theme || "", points: own };
}

/** During the sermon: the theme and its points as big buttons. Tap the point the predikant is at:
 *  the boards mark it and the stream shows it below the predikant for a moment. Tap the theme
 *  for the whole theme again. The one on the boards is gold. */
function Points({ slide, board }: { slide: Slide; board?: Slide }) {
  const { theme, points } = sermonTheme(slide, board);
  if (!points.length) return null;
  const on = slide.active ?? null;
  const look = (here: boolean) =>
    here ? "border-primary bg-primary text-primary-foreground shadow-[0_8px_24px_-14px_rgb(90_62_43/0.7)]" : "border-input bg-card hover:bg-accent";
  // Three points or more side by side in two columns when there is room, so all fit in view;
  // there the gold alone says which is on the boards.
  const two = points.length > 2;
  const mark = (narrow: boolean) => (
    <span className={cn("hymnal ml-auto shrink-0 pl-2 text-[18px] max-sm:hidden", narrow && "min-[1100px]:hidden")}>op het bord</span>
  );
  return (
    <div role="group" aria-label="Punten van het thema" className={cn("mt-4 grid gap-2", two && "min-[1100px]:grid-cols-2")}>
      <button type="button" aria-pressed={on === null} onClick={() => setPoint(null)} className={cn(pointButton, "col-span-full text-[26px]", look(on === null))}>
        <span className="hymnal shrink-0 text-[20px]">thema</span>
        <span className="min-w-0 font-semibold">{theme || "Heel het thema"}</span>
        {on === null && mark(false)}
      </button>
      {points.map((text, i) => (
        <button key={i} type="button" aria-pressed={on === i} onClick={() => setPoint(i)} className={cn(pointButton, "text-[30px]", look(on === i))}>
          <span className="w-9 shrink-0 text-right font-bold tabular-nums">{i + 1}.</span>
          <span className="min-w-0 font-semibold">{text}</span>
          {on === i && mark(two)}
        </button>
      ))}
    </div>
  );
}

/** The order of the service: done parts grey, the current one large on parchment, the next marked.
 *  It scrolls to the current part by itself and can be scrolled by hand. Tap another part to go
 *  there (after a yes); during the sermon the theme's points are in the current part. */
function Order({ svc, p, sub, hint, board }: { svc?: Service; p: ReturnType<typeof position>; sub: string; hint?: number; board?: Slide }) {
  const box = useRef<HTMLDivElement>(null);
  const [ask, setAsk] = useState<number | null>(null);
  useEffect(() => {
    const b = box.current, el = b?.querySelector<HTMLElement>("[data-now]");
    if (!b || !el) return;
    // The current part a third down, or higher when it is tall (the sermon with its points),
    // so all of it is in view; its top always is.
    const third = el.offsetTop - b.clientHeight / 3 + 8;
    const whole = el.offsetTop + el.offsetHeight + 40 - b.clientHeight;
    b.scrollTo({ top: Math.max(0, Math.min(Math.max(third, whole), el.offsetTop - 16)), behavior: "smooth" });
  }, [p.index, p.cur, svc?.items?.length]);

  if (!svc) return <div className="grid flex-1 place-items-center text-[28px] text-muted-foreground">Geen dienst</div>;
  const first = (i: number) => p.slides.findIndex((s) => s.item === i);
  const sermon = p.slide?.kind === "preek" ? p.slide : undefined;
  return (
    <div ref={box} className="relative min-h-0 flex-1 cursor-default overflow-y-auto overscroll-contain px-8 py-4 select-none [scrollbar-width:none] max-md:px-3 max-md:py-2 [mask-image:linear-gradient(to_bottom,transparent,#000_28px,#000_calc(100%-36px),transparent)] [&::-webkit-scrollbar]:hidden">
      <div className="grid gap-1 pb-[60vh] max-md:grid-cols-[minmax(0,1fr)]">
        {svc.items.map((it: Item, i: number) => {
          const label = itemLabel(it), detail = itemDetail(it);
          const ring = hint === i && i !== p.cur && "animate-hint";
          if (i === p.cur)
            return (
              <div key={it.id || i} data-now className="my-2 rounded-2xl border-2 border-[#e2cf9f] border-l-[6px] border-l-primary bg-[#fdf8ec] px-6 py-4 shadow-[0_10px_30px_-18px_rgb(90_62_43/0.6)] max-md:my-1 max-md:px-3 max-md:py-2.5">
                <div className="flex items-baseline gap-6 max-md:flex-wrap max-md:gap-x-3 max-md:gap-y-0">
                  <span className="shrink-0 text-[40px] leading-tight font-bold tracking-wide uppercase max-md:text-[26px]">▶&nbsp;&nbsp;{label}</span>
                  {detail && <span className="min-w-0 truncate text-[32px] leading-tight max-md:pl-[34px] max-md:text-[22px]">{detail}</span>}
                </div>
                {sub && <div className="mt-1 pl-[52px] text-[28px] leading-tight tabular-nums text-[#5a4632] max-md:pl-[34px] max-md:text-[20px]">{sub}</div>}
                {sermon && <Points slide={sermon} board={board} />}
              </div>
            );
          // Every other part: tap to go there.
          const row = (look: string, children: React.ReactNode) => (
            <button
              key={it.id || i}
              type="button"
              disabled={first(i) < 0}
              onClick={() => setAsk(i)}
              className={cn("flex w-full items-baseline rounded-xl text-left transition hover:bg-accent/60 disabled:hover:bg-transparent", look, ring)}
            >
              {children}
            </button>
          );
          if (p.cur >= 0 && i < p.cur)
            return row("gap-5 py-1 pl-2 text-[24px] text-done max-md:gap-2 max-md:pl-0 max-md:text-[19px]", <>
              <span className="w-7 shrink-0 max-md:w-5">✓</span>
              <span className="shrink-0">{label}</span>
              {detail && <span className="min-w-0 truncate">{detail}</span>}
            </>);
          if (i === p.nxt)
            return row("gap-5 rounded-l-none border-l-[3px] border-primary py-1.5 pl-5 text-[30px] max-md:flex-wrap max-md:gap-x-2.5 max-md:gap-y-0 max-md:pl-3 max-md:text-[22px]", <>
              <span className="hymnal shrink-0 text-[20px] text-[#7a5a1c] max-md:text-[18px]">hierna</span>
              <span className="shrink-0 font-semibold">{label}</span>
              {detail && <span className="min-w-0 truncate">{detail}</span>}
            </>);
          return row("gap-5 py-1 pl-[2.6rem] text-[26px] max-md:gap-2.5 max-md:pl-7 max-md:text-[20px]", <>
            <span className="shrink-0">{label}</span>
            {detail && <span className="min-w-0 truncate text-[#4a3826]">{detail}</span>}
          </>);
        })}
      </div>
      <Confirm
        open={ask !== null}
        onOpenChange={(o) => !o && setAsk(null)}
        title={`Naar ${itemLabel(svc.items[ask ?? -1])}?`}
        text={itemDetail(svc.items[ask ?? -1]) || undefined}
        yes="Ja"
        onYes={() => {
          const s = ask === null ? -1 : first(ask);
          if (s >= 0) go({ index: s });
        }}
      />
    </div>
  );
}

const big = "flex h-[112px] w-[880px] max-w-full min-w-0 flex-col items-center justify-center rounded-2xl px-8 transition active:scale-[0.99]";

/** Terug, and the one big button that moves the service on; at the end: wait or stop. */
function Bottom({ state, svc, p, status, now }: { state: View | null; svc?: Service; p: ReturnType<typeof position>; status: StreamStatus; now: number }) {
  const phone = usePhone();
  const [ask, setAsk] = useState<"" | "early" | "stop">("");
  const [jump, setJump] = useState(false);
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

  const confirms = (
    <>
      <Confirm open={ask === "early"} onOpenChange={(o) => !o && setAsk("")} title="Dienst begint al?" yes="Ja" onYes={() => step(1)} tone="gold" />
      <Confirm open={ask === "stop"} onOpenChange={(o) => !o && setAsk("")} title="Uitzending stoppen?" yes="Stoppen" no="Nog niet" onYes={() => setStreamMode("stop")} />
    </>
  );

  if (phone) {
    // On a phone: everything for the service at the bottom, under the thumb. Black and jumping
    // to a part sit on a small row above Terug and the big button.
    const quick = "flex h-12 min-w-0 items-center justify-center gap-2 rounded-xl border-2 text-[19px] font-semibold transition active:scale-[0.98] [&_svg]:size-5 [&_svg]:shrink-0";
    const black = !!state?.black;
    return (
      <footer className="grid shrink-0 gap-2 border-t-2 border-walnut/15 bg-[#efe5cc] px-2 pt-2 pb-[max(0.5rem,env(safe-area-inset-bottom))]">
        <div className="grid grid-cols-2 gap-2">
          <button
            onClick={() => go({ kind: "zwart" })}
            className={cn(quick, black ? "border-walnut bg-walnut text-white" : "border-input bg-card hover:bg-accent")}
          >
            {black ? <><Eye /> Borden weer aan</> : <><EyeOff /> Borden zwart</>}
          </button>
          <button disabled={!svc} onClick={() => setJump(true)} className={cn(quick, "border-input bg-card hover:bg-accent disabled:opacity-40")}>
            <ListOrdered /> <span className="truncate">Naar onderdeel</span>
          </button>
        </div>
        <div className="flex gap-2">
          <button
            disabled={p.index <= 0 || !p.slides.length}
            onClick={() => step(-1)}
            className="flex h-[76px] w-[92px] shrink-0 flex-col items-center justify-center rounded-2xl border-2 border-input bg-card font-semibold transition hover:bg-accent active:scale-[0.98] disabled:border-[#d8cbb0] disabled:bg-transparent disabled:text-[#a8987f]"
          >
            <span className="text-[28px] leading-none">←</span>
            <span className="text-[18px] leading-tight">Terug</span>
          </button>
          {einde ? (
            live ? (
              <>
                <button onClick={() => later()} className="h-[76px] min-w-0 flex-1 rounded-2xl border-2 border-input bg-card text-[21px] leading-tight font-semibold transition hover:bg-accent active:scale-[0.98]">
                  Nog 5 min
                </button>
                <button onClick={() => setAsk("stop")} className="h-[76px] min-w-0 flex-1 rounded-2xl bg-walnut text-[22px] leading-tight font-bold text-white transition hover:bg-[#6b4a33] active:scale-[0.98]">
                  Nu stoppen
                </button>
              </>
            ) : (
              <div className="flex-1" />
            )
          ) : (
            <button
              disabled={!p.slides.length}
              onClick={forward}
              className={cn(
                "flex h-[76px] min-w-0 flex-1 flex-col items-center justify-center rounded-2xl px-3 transition active:scale-[0.99]",
                "bg-primary text-primary-foreground shadow-[0_10px_30px_-12px_rgb(90_62_43/0.5)] hover:bg-primary-hover disabled:opacity-40",
                call && "animate-call",
              )}
            >
              <span className="text-[30px] leading-none font-bold whitespace-nowrap">{label}</span>
              {next && <span className="mt-1.5 max-w-full truncate text-[18px] leading-none">{next}</span>}
            </button>
          )}
        </div>
        <GotoSheet open={jump} onOpenChange={setJump} state={state} svc={svc} />
        {confirms}
      </footer>
    );
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
      {confirms}
    </footer>
  );
}

export function Dienst({ state, svc, status, lis }: Props) {
  const phone = usePhone();
  const [edit, setEdit] = useState(false);
  // On a phone the boards take the place of the list; on a wider screen they stand beside it.
  const [boards, setBoards] = useState(false);
  const showBoards = phone && boards;
  const now = Date.now();
  const p = position(state, svc);
  const sub = subline(p, svc, status, now);
  const s = lis?.suggestion?.slide;
  const hint = s !== undefined && s !== null ? state?.slides?.[s]?.item : undefined;
  return (
    <>
      <div className="flex min-h-0 flex-1">
        <div className="flex min-w-0 flex-1 flex-col">
          <div className="flex h-[72px] shrink-0 items-center gap-5 px-5 pt-1 max-md:h-auto max-md:flex-wrap max-md:gap-x-2 max-md:gap-y-1 max-md:px-2 max-md:pt-2 max-md:pb-1">
            <div role="radiogroup" aria-label="Lijst" className="flex shrink-0 rounded-2xl border-2 border-input bg-card p-1 max-md:min-w-0 max-md:flex-1">
              {([[false, "Overzicht", <ListChecks key="o" />], [true, "Bewerken", <Pencil key="e" />]] as const).map(([on, label, icon]) => (
                <button
                  key={label}
                  role="radio"
                  aria-checked={edit === on}
                  disabled={on && !svc}
                  onClick={() => { setEdit(on); setBoards(false); }}
                  className={cn(
                    "flex h-14 items-center gap-2 rounded-xl px-6 text-[22px] font-semibold transition [&_svg]:size-6 max-md:h-12 max-md:min-w-0 max-md:flex-1 max-md:justify-center max-md:gap-1.5 max-md:px-2 max-md:text-[19px] max-md:[&_svg]:size-5",
                    edit === on ? (on ? "bg-primary text-primary-foreground" : "bg-walnut text-white") : "text-muted-foreground hover:bg-accent",
                  )}
                >
                  {icon} {label}
                </button>
              ))}
            </div>
            {phone && (
              <button
                data-phone-tab="borden"
                aria-pressed={boards}
                onClick={() => setBoards(!boards)}
                className={cn(
                  "flex h-[60px] shrink-0 items-center gap-1.5 rounded-2xl border-2 px-3 text-[19px] font-semibold transition [&_svg]:size-5",
                  boards ? "border-primary bg-[#fdf8ec] text-foreground" : "border-input bg-card text-muted-foreground hover:bg-accent",
                )}
              >
                <LayoutGrid /> Borden
              </button>
            )}
            {edit && <span className="min-w-0 truncate text-[22px] text-[#7a5a1c] max-md:basis-full max-md:text-[18px] max-md:whitespace-normal">Bewerken — er verandert niets op het scherm</span>}
          </div>
          {showBoards ? (
            <BoardColumn state={state} svc={svc} edit={edit} phone />
          ) : edit && svc ? (
            <div className="flex min-h-0 flex-1 flex-col px-3 pb-3 max-md:px-1 max-md:pb-1">
              <OrderEdit svc={svc} cur={p.cur} />
            </div>
          ) : (
            <Order svc={svc} p={p} sub={sub} hint={hint} board={state?.boards?.[p.index]} />
          )}
        </div>
        {!phone && <BoardColumn state={state} svc={svc} edit={edit} />}
      </div>
      <Bottom state={state} svc={svc} p={p} status={status} now={now} />
    </>
  );
}
