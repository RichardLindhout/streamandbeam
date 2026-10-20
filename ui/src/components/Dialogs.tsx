import { useEffect, useState } from "react";
import { CalendarPlus, Globe, LogIn } from "lucide-react";
import { toast } from "sonner";
import type { Settings, StreamStatus, View } from "@/lib/api";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "./ui/tabs";
import { BoardBackground, StreamControls } from "./PrepareCards";
import { errorText, post } from "@/lib/api";
import { Dialog, DialogBody, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "./ui/dialog";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Label } from "./ui/label";
import { Switch } from "./ui/switch";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "./ui/select";

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="grid gap-3">
      <h4 className="font-serif text-[24px] font-semibold">{title}</h4>
      {children}
    </section>
  );
}

export function SettingsDialog({ open, onOpenChange, state, status, onMade }: {
  open: boolean; onOpenChange: (b: boolean) => void; state: View | null; status: StreamStatus; onMade?: () => void;
}) {
  const [s, setS] = useState<Settings | null>(null);
  const [yt, setYt] = useState<{ configured: boolean; channel: string } | null>(null);
  const [error, setError] = useState("");
  const [fetching, setFetching] = useState(false);
  const [newOpen, setNewOpen] = useState(false);
  useEffect(() => {
    if (!open) return;
    setError("");
    fetch("/api/settings")
      .then((r) => r.json())
      .then((v: Settings) =>
        // Older programs keep minutes after the end; show them as seconds.
        setS({ ...v, autoStopSeconds: v.autoStopSeconds ?? (v.autoStopMinutes !== undefined ? v.autoStopMinutes * 60 : 0) }),
      )
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
        </TabsList>
        <TabsContent value="uitzending"><DialogBody><StreamControls status={status} /></DialogBody></TabsContent>
        <TabsContent value="bord"><DialogBody><BoardBackground state={state} /></DialogBody></TabsContent>
        <TabsContent value="algemeen" className="flex min-h-0 flex-col">
        {s ? (
          <DialogBody className="gap-8">
            <Section title="Diensten">
              <div className="grid gap-3 sm:grid-cols-2">
                <Button variant="outline" disabled={fetching} onClick={fetchSite}><Globe /> {fetching ? "Bezig…" : "Diensten ophalen van website"}</Button>
                <Button variant="outline" onClick={() => setNewOpen(true)}><CalendarPlus /> Nieuwe dienst</Button>
              </div>
            </Section>
            <Section title="Camera en geluid">
              <div className="grid gap-3 sm:grid-cols-2">
                {f("cameraIp", "IP-adres camera (leeg = testbeeld)")}
                {f("cameraRtsp", "Camera RTSP-adres (leeg = standaard)")}
                {f("audioSource", "Geluidsbron (leeg = standaard)")}
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
              const r = await post("/api/settings", s, { quiet: true });
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
      <NewServiceDialog open={newOpen} onOpenChange={setNewOpen} onMade={onMade} />
    </Dialog>
  );
}

const KINDS = ["Morgendienst", "Middagdienst", "Avonddienst", "Rouwdienst", "Huwelijksdienst", "Bevestigingsdienst", "Doopdienst", "Heilig Avondmaal", "Voorbereiding Heilig Avondmaal", "Dankzegging Heilig Avondmaal", "Bidstond", "Dankdag", "Kerstfeest", "Oudejaarsavond", "Goede Vrijdag", "Pasen", "Hemelvaart", "Pinksteren", "Kinderzangdienst"];

function nextSunday() {
  const d = new Date();
  d.setDate(d.getDate() + ((7 - d.getDay()) % 7));
  const p = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
}

export function NewServiceDialog({ open, onOpenChange, onMade }: { open: boolean; onOpenChange: (b: boolean) => void; onMade?: () => void }) {
  const [v, setV] = useState({ date: nextSunday(), time: "09:30", label: "Morgendienst" });
  const [error, setError] = useState("");
  useEffect(() => { if (open) setError(""); }, [open]);
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Nieuwe dienst</DialogTitle>
          <DialogDescription className="sr-only">Datum, tijd en soort</DialogDescription>
        </DialogHeader>
        <DialogBody>
          <div className="grid grid-cols-2 gap-3">
            <div className="grid gap-1.5"><Label htmlFor="nd">Datum</Label><Input id="nd" type="date" value={v.date} onChange={(e) => setV({ ...v, date: e.target.value })} /></div>
            <div className="grid gap-1.5"><Label htmlFor="nt">Tijd</Label><Input id="nt" type="time" value={v.time} onChange={(e) => setV({ ...v, time: e.target.value })} /></div>
          </div>
          <div className="grid gap-1.5">
            <Label htmlFor="nl">Soort dienst</Label>
            <Input id="nl" list="kinds" value={v.label} onChange={(e) => setV({ ...v, label: e.target.value })} />
            <datalist id="kinds">{KINDS.map((k) => <option key={k} value={k} />)}</datalist>
          </div>
          {error && <div className="rounded-xl bg-[#fbe9e7] px-4 py-3 text-[22px] font-semibold text-live">{error}</div>}
        </DialogBody>
        <DialogFooter>
          <Button variant="ghost" onClick={() => onOpenChange(false)}>Annuleer</Button>
          <Button
            onClick={async () => {
              const r = await post("/api/new-service", v, { quiet: true });
              if (r.status === 499) return;
              if (!r.ok) return setError(r.status === 409 ? "Er is al een dienst op dat moment" : await errorText(r));
              onOpenChange(false);
              onMade?.();
            }}
          >
            Maak dienst
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
