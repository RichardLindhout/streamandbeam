import { useEffect, useState } from "react";
import { CalendarPlus, FileSpreadsheet, Globe, LogIn } from "lucide-react";
import { toast } from "sonner";
import type { ImportReport, ScheduleDay, ScheduleSummary, Schedules, Service, Settings, Span, StreamStatus, View } from "@/lib/api";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "./ui/tabs";
import { BoardBackground, StreamControls } from "./PrepareCards";
import { SoundSourcePicker } from "./SoundSource";
import { dayMonthYear, errorText, post, shortDate, shortDay, today } from "@/lib/api";
import { cn } from "@/lib/utils";
import { Dialog, DialogBody, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "./ui/dialog";
import { Button, buttonVariants } from "./ui/button";
import { Input } from "./ui/input";
import { Label } from "./ui/label";
import { Switch } from "./ui/switch";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "./ui/select";
import { sortedServices } from "./ServiceTiles";

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="grid gap-3">
      <h4 className="font-serif text-[24px] font-semibold">{title}</h4>
      {children}
    </section>
  );
}

export function SettingsDialog({ open, onOpenChange, state, status, onMade }: {
  open: boolean; onOpenChange: (b: boolean) => void; state: View | null; status: StreamStatus; onMade?: (id: string) => void;
}) {
  const [s, setS] = useState<Settings | null>(null);
  // The sound source as saved, to say that a new one counts at once.
  const [savedSource, setSavedSource] = useState<string | undefined>(undefined);
  const [yt, setYt] = useState<{ configured: boolean; channel: string } | null>(null);
  const [error, setError] = useState("");
  const [fetching, setFetching] = useState(false);
  const [newOpen, setNewOpen] = useState(false);
  useEffect(() => {
    if (!open) return;
    setError("");
    fetch("/api/settings")
      .then((r) => r.json())
      .then((v: Settings) => {
        // Older programs keep minutes after the end; show them as seconds.
        setS({ ...v, autoStopSeconds: v.autoStopSeconds ?? (v.autoStopMinutes !== undefined ? v.autoStopMinutes * 60 : 0) });
        setSavedSource(v.audioSource);
      })
      .catch(() => setError("Geen verbinding met de kerk-PC"));
    fetch("/api/youtube").then((r) => r.json()).then(setYt).catch(() => {});
  }, [open]);

  async function fetchSite() {
    setFetching(true);
    const r = await post("/api/fetch");
    setFetching(false);
    if (r.ok) toast.success("Diensten opgehaald");
  }

  const f = (k: string, label: string, type: "text" | "number" | "password" = "text") =>
    s && (
      <div className="grid gap-1.5">
        <Label htmlFor={k}>{label}</Label>
        <Input
          id={k}
          type={type}
          value={s[k] as string | number}
          onChange={(e) => setS({ ...s, [k]: type === "number" ? +e.target.value : e.target.value })}
        />
      </div>
    );
  const sel = (k: string, label: string, opts: [string, string][]) =>
    s && (
      <div className="grid gap-1.5">
        <Label>{label}</Label>
        <Select value={s[k] as string} onValueChange={(v) => setS({ ...s, [k]: v })}>
          <SelectTrigger><SelectValue /></SelectTrigger>
          <SelectContent>{opts.map(([v, l]) => <SelectItem key={v} value={v}>{l}</SelectItem>)}</SelectContent>
        </Select>
      </div>
    );

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-3xl">
        <DialogHeader className="border-b-0 pb-2">
          <DialogTitle>Instellingen</DialogTitle>
          <DialogDescription className="sr-only">Instellingen</DialogDescription>
        </DialogHeader>
        <Tabs defaultValue="algemeen" className="flex min-h-0 flex-col">
        <TabsList>
          <TabsTrigger value="algemeen">Algemeen</TabsTrigger>
          <TabsTrigger value="uitzending">Uitzending</TabsTrigger>
          <TabsTrigger value="bord">Bord</TabsTrigger>
          <TabsTrigger value="roosters">Roosters</TabsTrigger>
        </TabsList>
        <TabsContent value="uitzending"><DialogBody><StreamControls status={status} /></DialogBody></TabsContent>
        <TabsContent value="bord"><DialogBody><BoardBackground state={state} /></DialogBody></TabsContent>
        <TabsContent value="roosters" className="flex min-h-0 flex-col"><DialogBody><ScheduleSettings /></DialogBody></TabsContent>
        <TabsContent value="algemeen" className="flex min-h-0 flex-col">
        {s ? (
          <DialogBody className="gap-8">
            <Section title="Diensten">
              <div className="grid gap-3 sm:grid-cols-2">
                <Button variant="outline" disabled={fetching} onClick={fetchSite}><Globe /> {fetching ? "Bezig…" : "Diensten ophalen van website"}</Button>
                <Button variant="outline" onClick={() => setNewOpen(true)}><CalendarPlus /> Nieuwe dienst</Button>
              </div>
            </Section>
            <Section title="Geluid en camera">
              <SoundSourcePicker value={s.audioSource} saved={savedSource} status={status} onChange={(v) => setS({ ...s, audioSource: v })} />
              <div className="grid gap-3 sm:grid-cols-2">
                {f("cameraIp", "IP-adres camera (leeg = testbeeld)")}
                {f("cameraRtsp", "Camera RTSP-adres (leeg = standaard)")}
                {f("videoEncoder", "Video-encoder")}
              </div>
              <div className="grid gap-1.5">
                <Label>Camerapositie overzicht / orgel / preekstoel / doopvont</Label>
                <div className="grid grid-cols-4 gap-2">
                  {(["overzicht", "orgel", "preekstoel", "doopvont"] as const).map((p) => (
                    <Input key={p} type="number" title={p} value={s.presets[p]} onChange={(e) => setS({ ...s, presets: { ...s.presets, [p]: +e.target.value } })} />
                  ))}
                </div>
              </div>
            </Section>
            <Section title="Tijdens de dienst">
              <div className="grid gap-3 sm:grid-cols-2">
                {f("introSeconds", "Aankondiging psalm (sec)", "number")}
                {f("themeSeconds", "Thema in beeld (sec)", "number")}
                {f("autoStartMinutes", "Automatisch live (min vooraf, 0 = uit)", "number")}
                {f("autoStopSeconds", "Stoppen na aftiteling (seconden)", "number")}
                {sel("autopilot", "Meeluisteren", [["uit", "Uit"], ["suggestie", "Suggestie"], ["automatisch", "Automatisch"]])}
                {f("qrLink", "Link in QR-code")}
              </div>
            </Section>
            <Section title="YouTube">
              <div className="flex flex-wrap items-center justify-between gap-3 rounded-xl border-2 bg-background/50 p-3 text-[20px]">
                <span>
                  {yt?.channel ? <>Ingelogd als <b>{yt.channel}</b></> : yt?.configured ? "Niet ingelogd" : "Niet ingesteld"}
                </span>
                {yt?.channel ? (
                  <Button variant="ghost" size="sm" onClick={async () => { await post("/api/youtube/logout"); setYt({ ...yt, channel: "" }); }}>Uitloggen</Button>
                ) : yt?.configured ? (
                  <Button asChild size="sm"><a href="/youtube/login"><LogIn /> Inloggen met Google</a></Button>
                ) : null}
              </div>
              <div className="grid gap-3 sm:grid-cols-2">
                {f("youtubeClientId", "Google client-ID")}
                {f("youtubeClientSecret", "Google client-geheim", "password")}
                {f("youtubeChannel", "YouTube-kanaal (ID) voor de geluidscontrole")}
                {sel("youtubePrivacy", "Zichtbaarheid", [["public", "Openbaar"], ["unlisted", "Verborgen (met link)"], ["private", "Privé"]])}
                {f("streamKey", "Of: YouTube-streamsleutel", "password")}
                {f("rtmpUrl", "RTMP-adres")}
              </div>
              <label className="flex min-h-14 items-center gap-3 text-[20px]">
                <Switch checked={s.youtubeTest} onCheckedChange={(v) => setS({ ...s, youtubeTest: v })} />
                Testuitzending
              </label>
            </Section>
            <Section title="Opnames">{f("recordDir", "Map voor opnames")}</Section>
          </DialogBody>
        ) : (
          <DialogBody><div className="text-[20px] text-muted-foreground">{error || "Laden…"}</div></DialogBody>
        )}
        {error && s && <div className="mx-7 mb-3 rounded-xl bg-[#fbe9e7] px-4 py-3 text-[22px] font-semibold text-live">{error}</div>}
        <DialogFooter>
          <Button variant="ghost" onClick={() => onOpenChange(false)}>Annuleer</Button>
          <Button
            disabled={!s}
            onClick={async () => {
              // settingsVersion goes back as it came: a save over someone else's newer one is refused (409).
              // While live a new sound source or camera is asked first (the stream falters).
              const r = await post("/api/settings", s, { quiet: true });
              if (r.status === 499) return;
              if (r.ok) onOpenChange(false);
              else setError(await errorText(r));
            }}
          >
            Opslaan
          </Button>
        </DialogFooter>
        </TabsContent>
        </Tabs>
      </DialogContent>
      <NewServiceDialog open={newOpen} onOpenChange={setNewOpen} onMade={onMade} services={state?.services} />
    </Dialog>
  );
}

/** "62 data, 1 jan 2026 t/m 31 dec 2026" */
function spanText(s: Span) {
  return s.aantal ? `${s.aantal} ${s.aantal === 1 ? "datum" : "data"}, ${dayMonthYear(s.van)} t/m ${dayMonthYear(s.tot)}` : "geen";
}

function SummaryRows({ s }: { s: ScheduleSummary }) {
  const rows: [string, Span][] = [["Collectes", s.collectes], ["Bijzondere dagen", s.dagen], ["Voorzang", s.voorzang], ["Bediening", s.bediening]];
  const now = today();
  return (
    <div className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-6 gap-y-1 text-[20px]">
      {rows.map(([name, span]) => (
        <div key={name} className="contents">
          <span className="font-semibold">{name}</span>
          {/* A schedule that has run out is marked: then it needs a newer workbook. */}
          <span className={cn(span.aantal && span.tot >= now ? "" : "font-semibold text-[#9a5a00]")}>
            {spanText(span)}{span.aantal > 0 && span.tot < now && " — verlopen"}
          </span>
        </div>
      ))}
    </div>
  );
}

/** The church's schedules from its Excel workbook (Streamupdate.xlsm): read in again, how far
 *  they go, and what is coming (special days, voorzang, collectes, who operates). */
function ScheduleSettings() {
  const [s, setS] = useState<Schedules | null>(null);
  const [busy, setBusy] = useState(false);
  const [report, setReport] = useState<ImportReport | null>(null);
  const load = () => fetch("/api/roosters").then((r) => r.json()).then(setS).catch(() => setS(null));
  useEffect(() => { load(); }, []);

  async function upload(file: File) {
    setBusy(true);
    try {
      const r = await fetch("/api/roosters?naam=" + encodeURIComponent(file.name), { method: "POST", headers: { "Content-Type": "application/octet-stream" }, body: file });
      if (!r.ok) toast.error(await errorText(r), { duration: 8000 });
      else {
        setReport(await r.json());
        toast.success("Roosters bijgewerkt");
        load();
      }
    } catch {
      toast.error("Geen verbinding met de kerk-PC");
    }
    setBusy(false);
  }

  const who = (m: string, a: string) => (m && a ? (m === a ? m : `${m} / ${a}`) : m ? `${m} (morgen)` : a ? `${a} (middag)` : "");
  return (
    <div className="grid gap-8">
      <Section title="Roosters">
        <div className="text-[20px] text-muted-foreground">
          {s ? <>{s.eigen ? "Ingelezen uit" : "Ingebouwd, uit"} <b className="text-foreground">{s.bron || "—"}</b>{s.ingelezen && ` op ${dayMonthYear(s.ingelezen.slice(0, 10))}`}</> : "Laden…"}
        </div>
        {s && <SummaryRows s={s.samenvatting} />}
        <label className={cn(buttonVariants({ variant: "outline" }), "cursor-pointer", busy && "pointer-events-none opacity-50")}>
          <FileSpreadsheet /> {busy ? "Bezig…" : "Roosters uit Excel bijwerken"}
          <input
            type="file"
            accept=".xlsx,.xlsm,application/vnd.openxmlformats-officedocument.spreadsheetml.sheet,application/vnd.ms-excel.sheet.macroEnabled.12"
            className="sr-only"
            onChange={(e) => { const f = e.target.files?.[0]; e.target.value = ""; if (f) upload(f); }}
          />
        </label>
        <div className="text-[18px] text-muted-foreground">
          Uit de tabbladen Collecterooster, Voorzang en Streamdienst. De collecte van elke dienst volgt het rooster, behalve wat je zelf hebt getypt.
        </div>
        {report && (
          <div className="grid gap-1 rounded-xl border-2 border-ok/40 bg-ok/5 p-3 text-[18px]">
            <div className="font-semibold">Ingelezen uit {report.bron}</div>
            {report.behouden.length > 0 && <div>Niet in dit bestand, blijft zoals het was: {report.behouden.join(", ")}</div>}
            {report.waarschuwingen.slice(0, 6).map((w) => <div key={w} className="text-[#7a4a00]">{w}</div>)}
            {report.waarschuwingen.length > 6 && <div className="text-muted-foreground">en nog {report.waarschuwingen.length - 6}</div>}
          </div>
        )}
      </Section>
      <Section title="Komende weken">
        {!s?.komend?.length && <div className="text-[20px] text-muted-foreground">Niets in de roosters voor de komende weken</div>}
        <div className="grid gap-2">
          {(s?.komend || []).map((u) => (
            <div key={u.datum} className="grid gap-0.5 rounded-xl border-2 bg-background/50 px-4 py-2.5 text-[18px]">
              <div className="flex flex-wrap items-baseline gap-x-3">
                <span className="text-[20px] font-semibold tabular-nums">{shortDay(u.datum)}</span>
                {u.dag && <span className="hymnal text-[20px] text-[#7a5a1c]">{u.dag}</span>}
              </div>
              {(u.voorzangMorgen || u.voorzangMiddag) && (
                <div>Voorzang: {[u.voorzangMorgen && `morgen ${u.voorzangMorgen}`, u.voorzangMiddag && `middag ${u.voorzangMiddag}`].filter(Boolean).join(" · ")}</div>
              )}
              {u.collectes.length > 0 && <div className="text-muted-foreground">Collecte: {u.collectes.join(" · ")}</div>}
              {who(u.bedieningMorgen, u.bedieningMiddag) && (
                <div>Bediening: {who(u.bedieningMorgen, u.bedieningMiddag)}{u.notities.length > 0 && <span className="text-muted-foreground"> · {u.notities.join(", ")}</span>}</div>
              )}
            </div>
          ))}
        </div>
      </Section>
    </div>
  );
}

const KINDS = ["Morgendienst", "Middagdienst", "Avonddienst", "Bezinningsuur", "Rouwdienst", "Trouwdienst", "Huwelijksdienst", "Bevestigingsdienst", "Doopdienst", "Heilig Avondmaal", "Voorbereiding Heilig Avondmaal", "Dankzegging Heilig Avondmaal", "Bidstond", "Dankdag", "Kerstfeest", "Oudejaarsavond", "Goede Vrijdag", "Pasen", "Hemelvaart", "Pinksteren", "Kinderzangdienst"];

/** One tap for the usual services: name and time. */
const MOMENTS: [string, string][] = [["Morgendienst", "09:30"], ["Middagdienst", "16:00"], ["Avonddienst", "18:30"]];

/** The usual name for a service at this time of day. */
function kindFor(time: string) {
  return time < "13:00" ? "Morgendienst" : time < "18:00" ? "Middagdienst" : "Avonddienst";
}

function nextSunday() {
  const d = new Date();
  d.setDate(d.getDate() + ((7 - d.getDay()) % 7));
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}

/** A new service: date, time and name (one tap for the usual ones; on a special day its name
 *  is suggested), empty with the voorzang from the schedule, or with the liturgy of another
 *  service (e.g. Sunday's afternoon service again on a Wednesday, to try things out). */
export function NewServiceDialog({ open, onOpenChange, onMade, services, copyFrom }: {
  open: boolean; onOpenChange: (b: boolean) => void; onMade?: (id: string) => void; services?: Service[]; copyFrom?: string;
}) {
  const [v, setV] = useState({ date: nextSunday(), time: "09:30", label: "Morgendienst", copyFrom: "" });
  // The volunteer chose the name: it no longer follows the date and time.
  const [own, setOwn] = useState(false);
  const [day, setDay] = useState<ScheduleDay | null>(null);
  const [error, setError] = useState("");
  useEffect(() => {
    if (!open) return;
    setError("");
    setOwn(false);
    setV((x) => ({ ...x, copyFrom: copyFrom || "" }));
  }, [open, copyFrom]);
  // What the schedules say about the chosen moment.
  useEffect(() => {
    if (!open || !v.date) return;
    let gone = false;
    fetch(`/api/roosters/dag?datum=${encodeURIComponent(v.date)}&tijd=${encodeURIComponent(v.time)}`)
      .then((r) => r.json())
      .then((d: ScheduleDay) => !gone && setDay(d))
      .catch(() => !gone && setDay(null));
    return () => { gone = true; };
  }, [open, v.date, v.time]);
  // On a special day its name is the suggestion, else the usual name for the time.
  useEffect(() => {
    if (!own) setV((x) => ({ ...x, label: day?.dag || kindFor(x.time) }));
  }, [day, own]);

  const list = sortedServices(services).reverse();
  const copying = !!v.copyFrom && list.some((s) => s.id === v.copyFrom);
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>Nieuwe dienst</DialogTitle>
          <DialogDescription className="sr-only">Datum, tijd en soort</DialogDescription>
        </DialogHeader>
        <DialogBody>
          <div className="grid grid-cols-3 gap-2">
            {MOMENTS.map(([name, time]) => (
              <Button
                key={name}
                variant="outline"
                className={cn("h-auto flex-col gap-0 py-3 leading-tight", v.label === name && v.time === time && "border-walnut bg-[#efe3c6]")}
                onClick={() => { setOwn(true); setV({ ...v, label: name, time }); }}
              >
                {name}
                <span className="text-[18px] font-normal tabular-nums">{time}</span>
              </Button>
            ))}
          </div>
          <div className="grid grid-cols-2 gap-3">
            <div className="grid gap-1.5"><Label htmlFor="nd">Datum</Label><Input id="nd" type="date" value={v.date} onChange={(e) => setV({ ...v, date: e.target.value })} /></div>
            <div className="grid gap-1.5"><Label htmlFor="nt">Tijd</Label><Input id="nt" type="time" value={v.time} onChange={(e) => setV({ ...v, time: e.target.value })} /></div>
          </div>
          <div className="grid gap-1.5">
            <Label htmlFor="nl">Soort dienst</Label>
            <Input id="nl" list="kinds" value={v.label} onChange={(e) => { setOwn(true); setV({ ...v, label: e.target.value }); }} />
            <datalist id="kinds">{[...(day?.dag && !KINDS.includes(day.dag) ? [day.dag] : []), ...KINDS].map((k) => <option key={k} value={k} />)}</datalist>
            {day?.dag && v.label !== day.dag && (
              <button type="button" className="justify-self-start rounded-lg px-1 text-[20px] text-[#7a5a1c] underline-offset-4 hover:underline" onClick={() => { setOwn(true); setV({ ...v, label: day.dag }); }}>
                Bijzondere dag: <b>{day.dag}</b> — als naam gebruiken
              </button>
            )}
          </div>
          <div className="grid gap-1.5">
            <Label>Liturgie overnemen van</Label>
            <Select value={copying ? v.copyFrom : "none"} onValueChange={(x) => setV({ ...v, copyFrom: x === "none" ? "" : x })}>
              <SelectTrigger><SelectValue /></SelectTrigger>
              <SelectContent>
                <SelectItem value="none">Niets: een lege dienst</SelectItem>
                {list.map((s) => <SelectItem key={s.id} value={s.id}>{shortDate(s)} · {s.time} {s.label}</SelectItem>)}
              </SelectContent>
            </Select>
          </div>
          {day && (day.voorzang || day.collectes.length > 0 || day.bediening) && (
            <div className="grid gap-1 rounded-xl border-2 bg-background/50 px-4 py-3 text-[18px]">
              <div className="font-semibold">Uit de roosters</div>
              {day.voorzang && <div>Voorzang: {day.voorzang}{copying && <span className="text-muted-foreground"> (de overgenomen liturgie houdt haar eigen voorzang)</span>}</div>}
              {day.collectes.length > 0 && <div>Collecte: {day.collectes.join(" · ")}</div>}
              {day.bediening && <div>Bediening: {day.bediening}</div>}
            </div>
          )}
          {error && <div className="rounded-xl bg-[#fbe9e7] px-4 py-3 text-[22px] font-semibold text-live">{error}</div>}
        </DialogBody>
        <DialogFooter>
          <Button variant="ghost" onClick={() => onOpenChange(false)}>Annuleer</Button>
          <Button
            disabled={!v.date || !v.time || !v.label.trim()}
            onClick={async () => {
              const body = { date: v.date, time: v.time, label: v.label.trim(), copyFrom: copying ? v.copyFrom : "" };
              const r = await post("/api/new-service", body, { quiet: true });
              if (r.status === 499) return;
              if (!r.ok) return setError(r.status === 409 ? "Er is al een dienst op dat moment" : await errorText(r));
              onOpenChange(false);
              onMade?.(`${v.date} ${v.time}`);
            }}
          >
            {copying ? "Maak kopie" : "Maak dienst"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
