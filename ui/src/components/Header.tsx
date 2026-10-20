import { useState } from "react";
import { CalendarDays, Ellipsis, ExternalLink, Eye, EyeOff, Globe, ListOrdered, LoaderCircle, Settings2, Square, Type, Wrench } from "lucide-react";
import type { Service, StreamStatus, View, YouTubeCheck } from "@/lib/api";
import { at, go, hhmm, longDate, post, setStreamMode } from "@/lib/api";
import { health } from "@/lib/problems";
import { itemLabel } from "@/lib/kinds";
import { cn } from "@/lib/utils";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Label } from "./ui/label";
import { Textarea } from "./ui/textarea";
import { Popover, PopoverContent, PopoverTrigger } from "./ui/popover";
import { Sheet, SheetContent } from "./ui/sheet";
import { Dialog, DialogBody, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "./ui/dialog";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator, DropdownMenuTrigger } from "./ui/dropdown-menu";
import { Confirm } from "./Confirm";
import { toast } from "sonner";
import { ServiceTiles } from "./ServiceTiles";

export type Screen = "dienst" | "voorbereiden";

const LIGHT = {
  ok: { word: "Alles in orde", dot: "#2E7D32" },
  warn: { word: "Let op", dot: "#E08A00" },
  bad: { word: "Probleem", dot: "#C62828" },
};

/** One light and one word. Tap for a few lines, read only. */
function StatusLight({ status, yt, connected, alarm }: { status: StreamStatus; yt: YouTubeCheck | null; connected: boolean; alarm: boolean }) {
  const h = health(status, yt, connected, alarm);
  const l = LIGHT[h.level];
  return (
    <Popover>
      <PopoverTrigger asChild>
        <button className="flex h-[72px] shrink-0 items-center gap-3 rounded-xl px-4 text-[22px] font-semibold transition hover:bg-accent">
          <span className="size-5 rounded-full" style={{ background: l.dot }} />
          {l.word}
        </button>
      </PopoverTrigger>
      <PopoverContent align="start" className="w-auto min-w-72 p-5">
        <div className="grid gap-3">
          {h.lines.map((x) => (
            <div key={x.label} className="flex items-center gap-4 text-[24px]">
              <span className="size-4 rounded-full" style={{ background: x.ok ? LIGHT.ok.dot : x.label.startsWith("Borden") ? LIGHT.warn.dot : LIGHT.bad.dot }} />
              <span className="flex-1">{x.label}</span>
              <span className="font-semibold">{x.ok ? "goed" : "niet goed"}</span>
            </div>
          ))}
        </div>
      </PopoverContent>
    </Popover>
  );
}

/** What YouTube itself says (checked every minute while live): live, and with sound? */
function youtubeLines(status: StreamStatus, yt: YouTubeCheck | null) {
  // Only a check made after going live counts; the first takes about a minute.
  const fresh = !!yt?.at && yt.at >= (status.since || 0);
  if (!fresh) return { mark: "", lines: [{ ok: null, text: "YouTube wordt gecontroleerd" }] };
  const lines = [
    yt!.live ? { ok: true, text: "Live op YouTube" } : { ok: false, text: "Nog niet te zien op YouTube" },
    yt!.live && yt!.audio ? { ok: true, text: "Geluid op YouTube" } : { ok: false, text: yt!.live ? "Geen geluid op YouTube" : "Geluid nog niet te horen" },
  ];
  return { mark: lines.every((l) => l.ok) ? "✓" : "⚠", lines };
}

/** The broadcast on YouTube: this service's own, else the channel's live page. */
function youtubeUrl(svc?: Service, yt?: YouTubeCheck | null) {
  if (svc?.youtubeId) return `https://www.youtube.com/watch?v=${encodeURIComponent(svc.youtubeId)}`;
  if (yt?.channel) return `https://www.youtube.com/channel/${encodeURIComponent(yt.channel)}/live`;
  return "https://www.youtube.com";
}

/** Live: a red pill; tap it to see what YouTube receives and to open it there. Not live: when it
 *  will go live by itself, and the button to go live now. */
function LiveArea({ status, svc, yt }: { status: StreamStatus; svc?: Service; yt: YouTubeCheck | null }) {
  const [ask, setAsk] = useState(false);
  if (status.mode === "live") {
    const y = youtubeLines(status, yt);
    return (
      <Popover>
        <PopoverTrigger asChild>
          <button className="flex h-14 shrink-0 items-center gap-2.5 rounded-full bg-live px-5 text-[22px] font-bold tracking-wide text-white tabular-nums transition hover:brightness-110">
            <span className="size-3 animate-pulse-dot rounded-full bg-white" /> LIVE {hhmm(at(status.since))}
            {y.mark && <span className={cn("grid size-8 place-items-center rounded-full text-[20px]", y.mark === "✓" ? "bg-white text-ok" : "bg-warn text-[#2b1d12]")}>{y.mark}</span>}
          </button>
        </PopoverTrigger>
        <PopoverContent align="end" className="w-auto min-w-[22rem] p-5">
          <div className="grid gap-3">
            {y.lines.map((l) => (
              <div key={l.text} className={cn("flex items-center gap-3 text-[24px]", l.ok === false && "font-semibold text-[#9a5a00]")}>
                <span className="grid w-7 place-items-center">{l.ok === null ? <LoaderCircle className="size-6 animate-spin" /> : l.ok ? "✓" : "⚠"}</span> {l.text}
              </div>
            ))}
            <Button asChild variant="outline" className="mt-2">
              <a href={youtubeUrl(svc, yt)} target="_blank" rel="noreferrer"><ExternalLink /> Openen op YouTube</a>
            </Button>
          </div>
        </PopoverContent>
      </Popover>
    );
  }
  const startAt = at(status.autoStartAt);
  // Today only the time; another day also the day ("Gaat live zo 09:15").
  const day = startAt && startAt.toDateString() !== new Date().toDateString() ? ["zo", "ma", "di", "wo", "do", "vr", "za"][startAt.getDay()] : "om";
  const word = startAt ? `Gaat live ${day} ${hhmm(startAt)}` : status.mode === "opnemen" ? "Opname" : status.mode === "voorbeeld" ? "Voorbeeld" : "Uit";
  return (
    <>
      <span className="shrink-0 text-[22px] whitespace-nowrap tabular-nums">{word}</span>
      <Button variant="outline" className="shrink-0 border-live px-6 text-[22px] text-live hover:bg-[#fbe9e7]" onClick={() => setAsk(true)}>
        Nu live
      </Button>
      <Confirm open={ask} onOpenChange={setAsk} title="Live gaan?" yes="Ja, live" tone="live" onYes={() => setStreamMode("live")} />
    </>
  );
}

/** A small picture of the stream; tap for the whole screen, tap again to close. */
function PreviewThumb({ status }: { status: StreamStatus }) {
  const [big, setBig] = useState(false);
  const on = status.mode !== "stop";
  return (
    <>
      <button
        aria-label="Beeld"
        onClick={() => on && setBig(true)}
        className={cn("mon h-[90px] w-[160px] shrink-0 rounded-lg ring-2", status.mode === "live" ? "ring-live" : "ring-walnut/30", !on && "cursor-default bg-walnut/80")}
      >
        {on && <img src="/preview.mjpg" alt="" />}
      </button>
      {big && (
        <div className="fixed inset-0 z-[60] cursor-pointer bg-black" onClick={() => setBig(false)}>
          <div className="mon h-full w-full">
            <img src="/preview.mjpg" alt="" />
          </div>
        </div>
      )}
    </>
  );
}

/** All parts of the service; tap one, confirm, and the boards and stream go there. */
function GotoSheet({ open, onOpenChange, state, svc }: { open: boolean; onOpenChange: (b: boolean) => void; state: View | null; svc?: Service }) {
  const [ask, setAsk] = useState<number | null>(null);
  const slides = state?.slides || [];
  const cur = slides[state?.index ?? 0]?.item;
  const first = (i: number) => slides.findIndex((s) => s.item === i);
  return (
    <>
      <Sheet open={open} onOpenChange={onOpenChange}>
        <SheetContent title="Naar onderdeel">
          <div className="grid gap-1">
            {(svc?.items || []).map((it, i) => (
              <button
                key={it.id || i}
                disabled={first(i) < 0}
                onClick={() => setAsk(i)}
                className={cn(
                  "flex min-h-[72px] items-center gap-4 rounded-xl px-5 text-left text-[26px] transition hover:bg-accent disabled:opacity-40",
                  i === cur && "bg-[#efe3c6] font-semibold",
                )}
              >
                <span className="min-w-0 flex-1 truncate">{itemLabel(it)}</span>
                {it.ref && it.kind !== "welkom" && it.kind !== "einde" && <span className="truncate text-[22px] text-muted-foreground">{it.ref}</span>}
              </button>
            ))}
          </div>
        </SheetContent>
      </Sheet>
      <Confirm
        open={ask !== null}
        onOpenChange={(o) => !o && setAsk(null)}
        title={`Naar ${itemLabel(svc?.items?.[ask ?? -1])}?`}
        yes="Ja"
        onYes={() => {
          const s = ask === null ? -1 : first(ask);
          if (s >= 0) go({ index: s });
          onOpenChange(false);
        }}
      />
    </>
  );
}

/** Own words on the boards in church (never in the stream). */
function FreeTextDialog({ open, onOpenChange }: { open: boolean; onOpenChange: (b: boolean) => void }) {
  const [t, setT] = useState({ title: "", text: "" });
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Eigen tekst op het bord</DialogTitle>
          <DialogDescription className="sr-only">Kop en tekst</DialogDescription>
        </DialogHeader>
        <DialogBody>
          <div className="grid gap-2"><Label htmlFor="ft-kop">Kop</Label><Input id="ft-kop" value={t.title} onChange={(e) => setT({ ...t, title: e.target.value })} /></div>
          <div className="grid gap-2"><Label htmlFor="ft-tekst">Tekst</Label><Textarea id="ft-tekst" rows={4} value={t.text} onChange={(e) => setT({ ...t, text: e.target.value })} /></div>
        </DialogBody>
        <DialogFooter>
          <Button
            size="lg"
            disabled={!t.title.trim() && !t.text.trim()}
            onClick={async () => {
              const r = await post("/api/board-override", { ...t, stream: false });
              if (r.ok) onOpenChange(false);
            }}
          >
            Op het bord zetten
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function OtherServiceDialog({ open, onOpenChange, state }: { open: boolean; onOpenChange: (b: boolean) => void; state: View | null }) {
  const [ask, setAsk] = useState<Service | null>(null);
  return (
    <>
      <Dialog open={open} onOpenChange={onOpenChange}>
        <DialogContent className="max-w-2xl">
          <DialogHeader>
            <DialogTitle>Andere dienst</DialogTitle>
            <DialogDescription className="sr-only">Kies een dienst</DialogDescription>
          </DialogHeader>
          <DialogBody>
            <ServiceTiles services={state?.services} active={state?.current} onPick={(s) => s.id !== state?.current && setAsk(s)} />
          </DialogBody>
        </DialogContent>
      </Dialog>
      <Confirm
        open={ask !== null}
        onOpenChange={(o) => !o && setAsk(null)}
        title={ask ? `Naar ${ask.label} van ${longDate(ask).toLowerCase()}?` : ""}
        yes="Ja"
        onYes={async () => {
          if (!ask) return;
          const r = await post("/api/select?id=" + encodeURIComponent(ask.id));
          if (r.ok) onOpenChange(false);
        }}
      />
    </>
  );
}

/** The ⋯ menu of the service screen. */
function MainMenu({ state, svc, status, setScreen, onSettings }: {
  state: View | null; svc?: Service; status: StreamStatus; setScreen: (s: Screen) => void; onSettings: () => void;
}) {
  const live = status.mode === "live";
  const [open, setOpen] = useState(false);
  const [dlg, setDlg] = useState<"" | "goto" | "text" | "other" | "stop">("");
  const close = (o: boolean) => !o && setDlg("");
  return (
    <>
      <DropdownMenu open={open} onOpenChange={setOpen} modal={false}>
        <DropdownMenuTrigger asChild>
          <Button variant="outline" size="icon" aria-label="Menu" className="shrink-0"><Ellipsis className="size-9" /></Button>
        </DropdownMenuTrigger>
        <DropdownMenuContent className="w-[26rem]">
          <DropdownMenuItem onSelect={() => setDlg("goto")}><ListOrdered /> Naar onderdeel…</DropdownMenuItem>
          <DropdownMenuItem onSelect={() => setDlg("text")}><Type /> Eigen tekst op het bord</DropdownMenuItem>
          <DropdownMenuItem onSelect={() => go({ kind: "zwart" })}>
            {state?.black ? <><Eye /> Borden weer aan</> : <><EyeOff /> Borden zwart</>}
          </DropdownMenuItem>
          <DropdownMenuSeparator />
          <DropdownMenuItem onSelect={() => setScreen("voorbereiden")}><Wrench /> Dienst voorbereiden</DropdownMenuItem>
          <DropdownMenuItem onSelect={() => setDlg("other")}><CalendarDays /> Andere dienst…</DropdownMenuItem>
          <DropdownMenuItem onSelect={onSettings}><Settings2 /> Instellingen</DropdownMenuItem>
          {live && (
            <>
              <DropdownMenuSeparator />
              <DropdownMenuItem onSelect={() => setDlg("stop")} className="font-semibold text-live focus:bg-[#fbe9e7] [&_svg]:text-live">
                <Square className="fill-current" /> Uitzending stoppen
              </DropdownMenuItem>
            </>
          )}
        </DropdownMenuContent>
      </DropdownMenu>
      <GotoSheet open={dlg === "goto"} onOpenChange={close} state={state} svc={svc} />
      <FreeTextDialog open={dlg === "text"} onOpenChange={close} />
      <OtherServiceDialog open={dlg === "other"} onOpenChange={close} state={state} />
      <Confirm
        open={dlg === "stop"}
        onOpenChange={close}
        title="Uitzending stoppen?"
        text="Kijkers zien daarna niets meer."
        yes="Ja, stoppen"
        tone="live"
        onYes={() => setStreamMode("stop")}
      />
    </>
  );
}

/** "vandaag 08:15", "gisteren 21:00", "za 17 okt 08:15" */
function when(t?: number) {
  const d = at(t);
  if (!d) return "";
  const day = new Date(d), today = new Date();
  day.setHours(0, 0, 0, 0);
  today.setHours(0, 0, 0, 0);
  const diff = Math.round((+today - +day) / 86_400_000);
  const name = diff === 0 ? "vandaag" : diff === 1 ? "gisteren" : `${["zo", "ma", "di", "wo", "do", "vr", "za"][d.getDay()]} ${d.getDate()} ${["jan", "feb", "mrt", "apr", "mei", "jun", "jul", "aug", "sep", "okt", "nov", "dec"][d.getMonth()]}`;
  return `${name} ${hhmm(d)}`;
}

/** Quietly under the service name: when the website was last read. Tap for what was not
 *  understood, a newer website version, and fetching now. */
function WebsiteInfo({ state, svc }: { state: View | null; svc?: Service }) {
  const [busy, setBusy] = useState(false);
  const last = when(state?.lastFetched);
  const problems = svc?.problems || [];
  const flag = !!state?.fetchError || problems.length > 0 || !!svc?.websiteChanged;
  async function fetchNow() {
    setBusy(true);
    const r = await post("/api/fetch");
    setBusy(false);
    if (r.ok) toast.success("Opgehaald");
  }
  return (
    <Popover>
      <PopoverTrigger asChild>
        <button className="flex max-w-full items-center gap-2 truncate rounded-lg px-2 text-[18px] text-muted-foreground transition hover:bg-accent">
          {flag && <span className="size-2.5 shrink-0 rounded-full bg-warn" />}
          website: {last || "nog niet opgehaald"}
        </button>
      </PopoverTrigger>
      <PopoverContent className="grid w-[30rem] gap-4 p-5 text-[20px]">
        {svc?.websiteChanged && <WebsiteChanged svc={svc} />}
        {problems.length > 0 && (
          <div className="grid gap-1">
            <div className="font-semibold">Niet herkend</div>
            {problems.map((p) => <div key={p} className="text-[#7a4a00]">{p}</div>)}
          </div>
        )}
        {state?.fetchError && (
          <div className="grid gap-1">
            <div className="font-semibold">Ophalen lukte niet</div>
            <div className="line-clamp-3 text-[18px] break-words text-muted-foreground">{state.fetchError}</div>
          </div>
        )}
        <Button variant="outline" disabled={busy} onClick={fetchNow}>
          {busy ? <LoaderCircle className="animate-spin" /> : <Globe />} Nu ophalen
        </Button>
      </PopoverContent>
    </Popover>
  );
}

/** The website has a newer version of a service the volunteer changed: take it over or not. */
export function WebsiteChanged({ svc }: { svc: Service }) {
  const choose = (take: boolean) => post("/api/website", { id: svc.id, take });
  return (
    <div className="grid gap-3 rounded-xl border-2 border-warn/50 bg-warn/10 p-3">
      <div className="font-semibold">Website gewijzigd</div>
      {svc.website?.summary && <div className="text-[18px]">{svc.website.summary}</div>}
      <div className="flex gap-3">
        <Button size="sm" onClick={() => choose(true)}>Overnemen</Button>
        <Button size="sm" variant="outline" onClick={() => choose(false)}>Negeren</Button>
      </div>
    </div>
  );
}

export function Header({ state, svc, status, yt, connected, alarm, setScreen, onSettings }: {
  state: View | null; svc?: Service; status: StreamStatus; yt: YouTubeCheck | null; connected: boolean; alarm: boolean;
  setScreen: (s: Screen) => void; onSettings: () => void;
}) {
  return (
    <header className="flex h-24 shrink-0 items-center gap-4 border-b-2 border-walnut/15 bg-[#efe5cc] px-4">
      <StatusLight status={status} yt={yt} connected={connected} alarm={alarm} />
      <div className="flex min-w-0 flex-1 flex-col items-center leading-tight">
        <div className="max-w-full truncate text-[22px]">{svc ? `${longDate(svc)} · ${svc.label}` : "Geen dienst"}</div>
        <WebsiteInfo state={state} svc={svc} />
      </div>
      <LiveArea status={status} svc={svc} yt={yt} />
      <PreviewThumb status={status} />
      <MainMenu state={state} svc={svc} status={status} setScreen={setScreen} onSettings={onSettings} />
    </header>
  );
}
