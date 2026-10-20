import { useState } from "react";
import { CalendarPlus, Ear, Globe, LoaderCircle, MonitorPlay, Cross, Tv, Volume2, Wrench, SquarePlay as Youtube } from "lucide-react";
import { toast } from "sonner";
import { post } from "@/lib/api";
import { Button } from "./ui/button";
import { Select, SelectContent, SelectItem, SelectTrigger } from "./ui/select";
import type { ListenStatus, Service, StreamStatus, View, YouTubeCheck } from "@/lib/api";
import { clock } from "@/lib/api";
import { cn } from "@/lib/utils";
import type { ReactNode } from "react";

export type Screen = "dienst" | "voorbereiden";

type Tone = "ok" | "warn" | "live" | "off";

function Light({ tone, icon, label, value, title }: { tone: Tone; icon: ReactNode; label: string; value?: ReactNode; title?: string }) {
  return (
    <div title={title} className="flex h-10 items-center gap-2 rounded-lg border bg-card/80 pr-3 pl-2.5 text-[13px] text-muted-foreground [&_svg]:size-4">
      <span
        className={cn(
          "size-2 rounded-full",
          tone === "ok" && "bg-ok shadow-[0_0_10px_var(--ok)]",
          tone === "warn" && "bg-warn shadow-[0_0_10px_var(--warn)]",
          tone === "live" && "animate-pulse-dot bg-live shadow-[0_0_10px_var(--live)]",
          tone === "off" && "bg-white/20",
        )}
      />
      {icon}
      <span className="hidden xl:inline">{label}</span>
      {value != null && <b className="font-semibold text-foreground">{value}</b>}
    </div>
  );
}

const months = ["jan", "feb", "mrt", "apr", "mei", "jun", "jul", "aug", "sep", "okt", "nov", "dec"];
const days = ["zo", "ma", "di", "wo", "do", "vr", "za"];
export function serviceDate(s: Service) {
  const d = new Date(s.date + "T12:00:00");
  if (isNaN(+d)) return s.date;
  const t = `${days[d.getDay()]} ${d.getDate()} ${months[d.getMonth()]}`;
  return t[0].toUpperCase() + t.slice(1);
}

/** Which service is on, always in sight: choose another, fetch them from the website, or make a new one. */
function ServicePicker({ state, svc, onNew }: { state: View | null; svc?: Service; onNew: () => void }) {
  const [fetching, setFetching] = useState(false);
  async function fetchSite() {
    setFetching(true);
    const r = await post("/api/fetch");
    setFetching(false);
    if (!r.ok) toast.error("Ophalen mislukt: " + (await r.text()));
    else toast.success("Diensten opgehaald van de website");
  }
  return (
    <div className="order-last flex w-full min-w-0 items-center gap-2 xl:order-none xl:w-auto xl:flex-1">
      <Select value={state?.current || ""} onValueChange={(id) => post("/api/select?id=" + encodeURIComponent(id))}>
        <SelectTrigger className="h-12 min-w-0 flex-1 xl:max-w-[420px] rounded-xl border-border bg-card py-1 text-left">
          {svc ? (
            <span className="min-w-0 leading-tight">
              <span className="block truncate font-serif text-[16px] font-semibold">{svc.label}{svc.predikant ? <span className="font-normal text-muted-foreground"> · {svc.predikant}</span> : null}</span>
              <span className="block truncate text-xs text-muted-foreground">{serviceDate(svc)} · {svc.time} uur</span>
            </span>
          ) : (
            <span className="text-muted-foreground">Kies een dienst</span>
          )}
        </SelectTrigger>
        <SelectContent className="min-w-[340px]">
          {(state?.services || []).map((s) => (
            <SelectItem key={s.id} value={s.id}>
              {serviceDate(s)} · {s.time} · {s.label}{s.predikant ? " · " + s.predikant : ""}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
      <Button variant="outline" className="h-12 rounded-xl px-3.5" onClick={fetchSite} disabled={fetching} title="Diensten ophalen van hhgtholen.nl">
        {fetching ? <LoaderCircle className="animate-spin" /> : <Globe />}
        <span className="hidden 2xl:inline">{fetching ? "Bezig…" : "Website"}</span>
      </Button>
      <Button variant="outline" className="h-12 rounded-xl px-3.5" onClick={onNew}>
        <CalendarPlus /> <span className="hidden sm:inline">Nieuwe dienst</span>
      </Button>
    </div>
  );
}

export function Header({ state, svc, status, lis, yt, screen, setScreen, onNew }: {
  state: View | null; svc?: Service; status: StreamStatus; lis: ListenStatus | null; yt: YouTubeCheck | null; screen: Screen; setScreen: (s: Screen) => void; onNew: () => void;
}) {
  const live = status.mode === "live";
  return (
    <header className="flex flex-wrap items-center gap-x-5 gap-y-3 border-b bg-[#0d0b09]/90 px-5 py-3 backdrop-blur">
      <div className="flex items-center gap-3">
        <div className="grid size-10 place-items-center rounded-xl border border-primary/45 bg-gradient-to-b from-[#2a231b] to-[#18130f] shadow-[0_8px_24px_-12px_rgb(201_164_92/0.6)]">
          <Cross className="size-5 fill-primary/25 text-primary" strokeWidth={1.75} />
        </div>
        <div className="hidden leading-tight xl:block">
          <div className="font-serif text-[16px] font-semibold tracking-tight">Stream &amp; Beam</div>
          <div className="text-xs text-muted-foreground">HHG Tholen</div>
        </div>
      </div>

      <div className="hidden h-8 w-px bg-border xl:block" />

      <ServicePicker state={state} svc={svc} onNew={onNew} />

      <div className="ml-auto flex flex-wrap items-center gap-2 xl:ml-0">
        <Light tone={status.boards > 0 ? "ok" : "warn"} icon={<Tv />} label="Borden" value={status.boards} title="Aantal verbonden borden in de kerk" />
        <Light tone={status.mode === "stop" ? "off" : status.audioWarning ? "warn" : "ok"} icon={<Volume2 />} label="Geluid" />
        {live && (
          <Light
            tone={!yt?.at ? "off" : yt.audio ? "ok" : "live"}
            icon={<Youtube />}
            label="YouTube-geluid"
            value={yt?.at ? (yt.audio ? "✓" : "⚠") : "…"}
            title={yt?.problem || "Wat kijkers op YouTube horen, elke minuut gemeten"}
          />
        )}
        {lis?.active && <Light tone="ok" icon={<Ear />} label="Meeluisteren" />}
        <div
          className={cn(
            "flex h-10 items-center gap-2 rounded-lg px-3 font-mono text-[13px] font-bold tracking-wider",
            live ? "bg-live text-[#fff6ef] shadow-[0_0_24px_-6px_var(--live)]" : status.mode === "opnemen" ? "bg-live/15 text-[#f0a0a4] ring-1 ring-live/40" : "border bg-card/80 text-muted-foreground",
          )}
        >
          <span className={cn("size-2 rounded-full", live ? "animate-pulse-dot bg-[#fff6ef]" : status.mode === "opnemen" ? "bg-live" : status.mode === "voorbeeld" ? "bg-ok" : "bg-white/20")} />
          {live ? <>LIVE {clock(status.since)}</> : status.mode === "opnemen" ? <>OPNAME {clock(status.since)}</> : status.mode === "voorbeeld" ? "VOORBEELD" : "NIET LIVE"}
        </div>
      </div>

      <nav className="flex rounded-xl border bg-card p-1">
        {([
          ["dienst", "Dienst", <MonitorPlay key="d" />],
          ["voorbereiden", "Voorbereiden", <Wrench key="v" />],
        ] as const).map(([k, label, icon]) => (
          <button
            key={k}
            onClick={() => setScreen(k)}
            className={cn(
              "flex h-9 items-center gap-2 rounded-lg px-4 text-sm font-semibold transition [&_svg]:size-4",
              screen === k ? "bg-primary text-primary-foreground shadow" : "text-muted-foreground hover:text-foreground",
            )}
          >
            {icon}
            {label}
          </button>
        ))}
      </nav>
    </header>
  );
}
