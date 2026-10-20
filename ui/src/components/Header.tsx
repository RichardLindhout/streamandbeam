import { useState } from "react";
import { ArrowLeft, CalendarPlus, Cross, Ellipsis, Eye, EyeOff, Globe, Settings2, Wrench } from "lucide-react";
import { toast } from "sonner";
import type { ListenStatus, Service, StreamStatus, View, YouTubeCheck } from "@/lib/api";
import { go, post } from "@/lib/api";
import type { Problem } from "@/lib/problems";
import { cn } from "@/lib/utils";
import { Button } from "./ui/button";
import { Select, SelectContent, SelectItem, SelectTrigger } from "./ui/select";
import { Popover, PopoverContent, PopoverTrigger } from "./ui/popover";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator, DropdownMenuTrigger } from "./ui/dropdown-menu";

export type Screen = "dienst" | "voorbereiden";

const months = ["jan", "feb", "mrt", "apr", "mei", "jun", "jul", "aug", "sep", "okt", "nov", "dec"];
const days = ["zo", "ma", "di", "wo", "do", "vr", "za"];
export function serviceDate(s: Service) {
  const d = new Date(s.date + "T12:00:00");
  if (isNaN(+d)) return s.date;
  const t = `${days[d.getDay()]} ${d.getDate()} ${months[d.getMonth()]}`;
  return t[0].toUpperCase() + t.slice(1);
}

/** Which service is on: one dropdown to choose another. */
function ServicePicker({ state, svc }: { state: View | null; svc?: Service }) {
  return (
    <Select value={state?.current || ""} onValueChange={(id) => post("/api/select?id=" + encodeURIComponent(id))}>
      <SelectTrigger className="h-12 w-auto max-w-[440px] min-w-0 flex-1 rounded-xl border-border bg-card py-1 text-left">
        {svc ? (
          <span className="min-w-0 leading-tight">
            <span className="block truncate font-serif text-[17px] font-semibold">{svc.label}{svc.predikant ? <span className="font-normal text-muted-foreground"> · {svc.predikant}</span> : null}</span>
            <span className="block truncate text-[13px] text-muted-foreground">{serviceDate(svc)} · {svc.time} uur</span>
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
  );
}

function Row({ ok, label, value }: { ok: boolean | null; label: string; value: string }) {
  return (
    <div className="flex items-center gap-3 py-1.5 text-sm">
      <span className={cn("size-2 rounded-full", ok === null ? "bg-white/20" : ok ? "bg-ok" : "bg-warn")} />
      <span className="flex-1 text-muted-foreground">{label}</span>
      <span className="font-semibold">{value}</span>
    </div>
  );
}

/** One light: green when all is well; amber or red with a short word when not. Tap for details. */
function StatusLight({ problems, status, lis, yt }: { problems: Problem[]; status: StreamStatus; lis: ListenStatus | null; yt: YouTubeCheck | null }) {
  const worst = problems.find((p) => p.severity === "rood") || problems[0];
  const running = status.mode !== "stop";
  return (
    <Popover>
      <PopoverTrigger asChild>
        <button
          className={cn(
            "flex h-12 items-center gap-2.5 rounded-xl border px-4 text-[15px] font-semibold transition",
            !worst && "border-ok/30 bg-ok/10 text-[#c9d6a6]",
            worst?.severity === "oranje" && "border-warn/40 bg-warn/10 text-warn",
            worst?.severity === "rood" && "border-live/50 bg-live/15 text-[#f3a3a8]",
          )}
        >
          <span className={cn("size-2.5 rounded-full", !worst ? "bg-ok shadow-[0_0_10px_var(--ok)]" : worst.severity === "rood" ? "bg-live" : "bg-warn")} />
          <span className="max-w-[220px] truncate">{worst ? worst.title : "Alles in orde"}</span>
        </button>
      </PopoverTrigger>
      <PopoverContent align="end" className="w-80">
        <div className="hymnal mb-2 text-[15px] text-primary">hoe staat het ervoor</div>
        <Row ok={status.boards > 0} label="Borden in de kerk" value={status.boards > 0 ? `${status.boards} verbonden` : "geen"} />
        <Row ok={running ? !status.audioWarning : null} label="Geluid van de kerk" value={!running ? "uitzending uit" : status.audioWarning ? "geen geluid" : "in orde"} />
        <Row
          ok={status.mode === "live" && yt?.at ? yt.audio : null}
          label="Geluid op YouTube"
          value={status.mode !== "live" ? "niet live" : !yt?.at ? "wordt gecontroleerd" : yt.audio ? "te horen" : "niet te horen"}
        />
        <Row ok={lis?.active ? true : null} label="Meeluisteren" value={lis?.active ? "aan" : "uit"} />
      </PopoverContent>
    </Popover>
  );
}

export function Header({ state, svc, status, lis, yt, problems, screen, setScreen, onNew, onSettings }: {
  state: View | null; svc?: Service; status: StreamStatus; lis: ListenStatus | null; yt: YouTubeCheck | null; problems: Problem[];
  screen: Screen; setScreen: (s: Screen) => void; onNew: () => void; onSettings: () => void;
}) {
  const [fetching, setFetching] = useState(false);
  async function fetchSite() {
    setFetching(true);
    const r = await post("/api/fetch");
    setFetching(false);
    if (!r.ok) toast.error("Ophalen is niet gelukt. Werkt het internet?");
  }
  return (
    <header className="flex flex-wrap items-center gap-3 border-b bg-[#0d0b09]/90 px-5 py-3 backdrop-blur">
      <div className="flex items-center gap-3 pr-2">
        <div className="grid size-11 place-items-center rounded-xl border border-primary/45 bg-gradient-to-b from-[#2a231b] to-[#18130f] shadow-[0_8px_24px_-12px_rgb(201_164_92/0.6)]">
          <Cross className="size-5 fill-primary/25 text-primary" strokeWidth={1.75} />
        </div>
        <div className="hidden leading-tight xl:block">
          <div className="font-serif text-[17px] font-semibold tracking-tight">Stream &amp; Beam</div>
          <div className="text-[13px] text-muted-foreground">HHG Tholen</div>
        </div>
      </div>

      <ServicePicker state={state} svc={svc} />

      <div className="ml-auto flex items-center gap-3">
        <StatusLight problems={problems} status={status} lis={lis} yt={yt} />
        <DropdownMenu>
          <DropdownMenuTrigger asChild>
            <Button variant="outline" className="h-12 rounded-xl px-4 text-[15px]"><Ellipsis /> Menu</Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent>
            {screen === "dienst" ? (
              <DropdownMenuItem onSelect={() => setScreen("voorbereiden")}><Wrench /> Dienst voorbereiden</DropdownMenuItem>
            ) : (
              <DropdownMenuItem onSelect={() => setScreen("dienst")}><ArrowLeft /> Terug naar de dienst</DropdownMenuItem>
            )}
            <DropdownMenuItem onSelect={onNew}><CalendarPlus /> Nieuwe dienst</DropdownMenuItem>
            <DropdownMenuItem disabled={fetching} onSelect={fetchSite}><Globe /> {fetching ? "Bezig met ophalen…" : "Diensten ophalen van website"}</DropdownMenuItem>
            <DropdownMenuSeparator />
            <DropdownMenuItem onSelect={() => go({ kind: "zwart" })}>
              {state?.black ? <><Eye /> Borden weer aan</> : <><EyeOff /> Borden zwart</>}
            </DropdownMenuItem>
            <DropdownMenuItem onSelect={onSettings}><Settings2 /> Instellingen</DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>
    </header>
  );
}
