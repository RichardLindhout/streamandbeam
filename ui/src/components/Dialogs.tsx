import { useEffect, useState } from "react";
import { LogIn } from "lucide-react";
import type { Settings } from "@/lib/api";
import { post } from "@/lib/api";
import { Dialog, DialogBody, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "./ui/dialog";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Label } from "./ui/label";
import { Switch } from "./ui/switch";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "./ui/select";

function Section({ title, children }: { title: string; children: React.ReactNode }) {
  return (
    <section className="grid gap-3">
      <h4 className="text-[11px] font-bold tracking-[0.14em] text-primary uppercase">{title}</h4>
      {children}
    </section>
  );
}

export function SettingsDialog({ open, onOpenChange }: { open: boolean; onOpenChange: (b: boolean) => void }) {
  const [s, setS] = useState<Settings | null>(null);
  const [yt, setYt] = useState<{ configured: boolean; channel: string } | null>(null);
  useEffect(() => {
    if (!open) return;
    fetch("/api/settings").then((r) => r.json()).then(setS);
    fetch("/api/youtube").then((r) => r.json()).then(setYt).catch(() => {});
  }, [open]);

  const f = (k: keyof Settings, label: string, type: "text" | "number" | "password" = "text") =>
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
  const sel = (k: keyof Settings, label: string, opts: [string, string][]) =>
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
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>Instellingen</DialogTitle>
          <DialogDescription>Camera, geluid, YouTube en het meeluisteren.</DialogDescription>
        </DialogHeader>
        {s ? (
          <DialogBody className="gap-7">
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
                {f("autoStopMinutes", "Automatisch stoppen (min na einde)", "number")}
                {sel("autopilot", "Meeluisteren", [["uit", "Uit"], ["suggestie", "Suggestie (knop kleurt)"], ["automatisch", "Automatisch als het zeker is"]])}
                {f("qrLink", "Link in QR-code")}
              </div>
            </Section>
            <Section title="YouTube">
              <div className="flex flex-wrap items-center justify-between gap-3 rounded-xl border bg-background/50 p-3 text-sm">
                <span className="text-muted-foreground">
                  {yt?.channel ? <>Ingelogd als <b className="text-foreground">{yt.channel}</b></> : yt?.configured ? "Nog niet ingelogd (alleen op deze PC)" : "Vul hieronder de Google-gegevens in, of gebruik een streamsleutel"}
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
              <label className="flex items-center gap-3 text-sm">
                <Switch checked={s.youtubeTest} onCheckedChange={(v) => setS({ ...s, youtubeTest: v })} />
                Testuitzending (privé, met “TEST” in de titel)
              </label>
            </Section>
            <Section title="Opnames">{f("recordDir", "Map voor opnames")}</Section>
          </DialogBody>
        ) : (
          <DialogBody><div className="text-sm text-muted-foreground">Laden…</div></DialogBody>
        )}
        <DialogFooter>
          <Button variant="ghost" onClick={() => onOpenChange(false)}>Annuleer</Button>
          <Button disabled={!s} onClick={async () => { await post("/api/settings", s); onOpenChange(false); }}>Opslaan</Button>
        </DialogFooter>
      </DialogContent>
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
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Nieuwe dienst</DialogTitle>
          <DialogDescription>Daarna typ je wat er komt, in elke volgorde — ook bij een rouw- of huwelijksdienst.</DialogDescription>
        </DialogHeader>
        <DialogBody>
          <div className="grid grid-cols-2 gap-3">
            <div className="grid gap-1.5"><Label htmlFor="nd">Datum</Label><Input id="nd" type="date" value={v.date} onChange={(e) => setV({ ...v, date: e.target.value })} /></div>
            <div className="grid gap-1.5"><Label htmlFor="nt">Tijd</Label><Input id="nt" type="time" value={v.time} onChange={(e) => setV({ ...v, time: e.target.value })} /></div>
          </div>
          <div className="grid gap-1.5">
            <Label htmlFor="nl">Soort dienst (vrij in te vullen)</Label>
            <Input id="nl" list="kinds" value={v.label} onChange={(e) => setV({ ...v, label: e.target.value })} />
            <datalist id="kinds">{KINDS.map((k) => <option key={k} value={k} />)}</datalist>
          </div>
        </DialogBody>
        <DialogFooter>
          <Button variant="ghost" onClick={() => onOpenChange(false)}>Annuleer</Button>
          <Button onClick={async () => { await post("/api/new-service", v); onOpenChange(false); onMade?.(); }}>Maak dienst</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
