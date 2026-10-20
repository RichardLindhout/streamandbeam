import { useState } from "react";
import { Circle, Eye, ImageUp, Mic, Radio, Square, Trash, TreePine, Type, Video, Image as ImageIcon } from "lucide-react";
import { toast } from "sonner";
import type { Mode, Service, StreamStatus, View } from "@/lib/api";
import { post, quick, saveService } from "@/lib/api";
import { cn } from "@/lib/utils";
import { Card, CardContent, CardHeader } from "./ui/card";
import { Button, buttonVariants } from "./ui/button";
import { Input } from "./ui/input";
import { Textarea } from "./ui/textarea";
import { Label } from "./ui/label";
import { Switch } from "./ui/switch";
import {
  AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle, AlertDialogTrigger,
} from "./ui/alert-dialog";
import { BlurField } from "./BlurField";
import { Board } from "./Board";
import { ConfirmMode, setStreamMode } from "./LiveButton";

function Field({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="grid gap-1.5">
      <Label>{label}</Label>
      {children}
    </div>
  );
}

export function SermonCard({ svc }: { svc: Service }) {
  const save = (patch: Partial<Service>) => saveService({ ...svc, ...patch });
  return (
    <Card>
      <CardHeader><Mic /> Preek</CardHeader>
      <CardContent className="grid gap-4">
        <Field label="Predikant"><BlurField value={svc.predikant} onSave={(v) => save({ predikant: v })} /></Field>
        <Field label="Tekst (bijv. joh 5:25 of HC 57)">
          <BlurField
            value={svc.text?.ref || ""}
            onSave={async (v) => {
              try {
                const [it] = await quick(v);
                if (it) save({ text: it });
              } catch { /* not recognised: keep the old text */ }
            }}
          />
        </Field>
        <Field label="Thema"><BlurField value={svc.theme} onSave={(v) => save({ theme: v })} /></Field>
        <Field label="Punten (één per regel)">
          <BlurField multiline rows={3} value={(svc.points || []).join("\n")} onSave={(v) => save({ points: v.split("\n").map((s) => s.trim()).filter(Boolean) })} />
        </Field>
        <div className="mt-1 flex items-center justify-between gap-3 rounded-xl border border-live/25 bg-live/[0.04] p-3">
          <span className="text-[13px] text-muted-foreground">Deze dienst helemaal weghalen</span>
          <AlertDialog>
            <AlertDialogTrigger asChild>
              <Button variant="outline" size="sm" className="border-live/40 text-[#ff8a95] hover:bg-live/15"><Trash /> Verwijderen</Button>
            </AlertDialogTrigger>
            <AlertDialogContent>
              <AlertDialogHeader>
                <AlertDialogTitle>Deze dienst verwijderen?</AlertDialogTitle>
                <AlertDialogDescription>“{svc.label}” van {svc.date} met de hele volgorde verdwijnt. Dit kan niet ongedaan worden.</AlertDialogDescription>
              </AlertDialogHeader>
              <AlertDialogFooter>
                <AlertDialogCancel>Annuleer</AlertDialogCancel>
                <AlertDialogAction className="bg-live text-[#fff6ef]" onClick={() => post("/api/delete-service?id=" + encodeURIComponent(svc.id))}>Verwijderen</AlertDialogAction>
              </AlertDialogFooter>
            </AlertDialogContent>
          </AlertDialog>
        </div>
      </CardContent>
    </Card>
  );
}

export function FreeTextCard({ state }: { state: View | null }) {
  const [t, setT] = useState({ title: "", text: "", stream: false });
  const has = t.title || t.text;
  return (
    <Card>
      <CardHeader><Type /> Vrije tekst op het bord</CardHeader>
      <CardContent className="grid gap-4 sm:grid-cols-[minmax(0,1fr)_110px]">
        <div className="grid gap-3">
          <Input placeholder="Bovenste regel (groot)" value={t.title} onChange={(e) => setT({ ...t, title: e.target.value })} />
          <Textarea rows={4} placeholder="Eén regel per richel" value={t.text} onChange={(e) => setT({ ...t, text: e.target.value })} />
          <label className="flex items-center gap-3 text-sm">
            <Switch checked={t.stream} onCheckedChange={(v) => setT({ ...t, stream: v })} /> ook in de uitzending
          </label>
          <div className="flex flex-wrap gap-2">
            <Button disabled={!has} onClick={() => post("/api/board-override", t)}>Op het bord zetten</Button>
            {state?.overrideBoard && <Button variant="outline" onClick={() => post("/api/board-override", { clear: true })}>Weghalen</Button>}
          </div>
        </div>
        <div className={cn("self-start", !has && "opacity-30")}>
          <Board slide={{ kind: "vrij", item: 0, sub: "", header: t.title, lines: t.text.split("\n") }} className="rounded-lg ring-1 ring-white/15" />
          <div className="mt-1.5 text-center text-[11px] text-muted-foreground">voorbeeld</div>
        </div>
      </CardContent>
    </Card>
  );
}

export function StreamCard({ status, onSettings }: { status: StreamStatus; onSettings: () => void }) {
  const [ask, setAsk] = useState<Mode | null>(null);
  const modes: [Mode, string, React.ReactNode][] = [
    ["voorbeeld", "Voorbeeld", <Eye key="v" />],
    ["opnemen", "Opnemen", <Circle key="o" className="fill-current text-live" />],
    ["live", "Live", <Radio key="l" />],
    ["stop", "Stop", <Square key="s" className="fill-current" />],
  ];
  const choose = (m: Mode) => {
    if (m === "live" || (m === "stop" && status.mode === "live")) setAsk(m);
    else setStreamMode(m);
  };
  return (
    <Card>
      <CardHeader action={<Button variant="ghost" size="sm" onClick={onSettings}>Instellingen</Button>}><Video /> Uitzending</CardHeader>
      <CardContent className="grid gap-3">
        <div className="grid grid-cols-2 gap-2 sm:grid-cols-4">
          {modes.map(([m, label, icon]) => (
            <Button
              key={m}
              variant="outline"
              size="lg"
              onClick={() => choose(m)}
              className={cn(
                status.mode === m && (m === "live" ? "border-live bg-live text-[#fff6ef] hover:bg-live" : "border-primary bg-primary/15 text-foreground"),
              )}
            >
              {icon} {label}
            </Button>
          ))}
        </div>
        {status.recording && <div className="truncate text-[13px] text-muted-foreground">Opname: {status.recording.split(/[\\/]/).pop()}</div>}
        {status.error && <div className="rounded-lg border border-warn/40 bg-warn/10 p-3 text-[13px] text-warn">{status.error}</div>}
      </CardContent>
      <ConfirmMode mode={ask} open={ask !== null} onOpenChange={(o) => !o && setAsk(null)} />
    </Card>
  );
}

/** The background picture of the boards in church: the wood, or a photo of the church's own. */
export function BoardBackgroundCard({ state }: { state: View | null }) {
  const [busy, setBusy] = useState(false);
  const sample = state?.boards?.find((b) => b.kind === "bord") || { kind: "bord", item: 0, header: "", sub: "", list: ["Ps 42:1", "Ps 119:5", "Joh. 3:16", "", "Ps 25:2"] };
  const custom = (state?.boardBackground ?? 0) > 0;
  async function upload(file: File) {
    setBusy(true);
    const r = await fetch("/api/bord-achtergrond", { method: "POST", headers: { "Content-Type": file.type }, body: file });
    setBusy(false);
    if (!r.ok) toast.error(await r.text());
    else toast.success("Nieuwe achtergrond op de borden");
  }
  return (
    <Card>
      <CardHeader><ImageIcon /> Achtergrond van het bord</CardHeader>
      <CardContent className="grid grid-cols-[110px_minmax(0,1fr)] items-start gap-5">
        <Board slide={sample} className="rounded-lg ring-1 ring-[#3a3229]" />
        <div className="grid gap-3">
          <p className="text-sm text-muted-foreground">
            {custom ? "De borden tonen nu een eigen afbeelding." : "De borden tonen nu het hout."} Een staande foto (9:16) past het best;
            de letters blijven wit en worden iets donkerder omrand.
          </p>
          <label className={cn(buttonVariants({ variant: "outline" }), "cursor-pointer", busy && "pointer-events-none opacity-50")}>
            <ImageUp /> {busy ? "Bezig…" : "Kies afbeelding…"}
            <input
              type="file"
              accept="image/jpeg,image/png,image/webp"
              className="sr-only"
              onChange={(e) => { const f = e.target.files?.[0]; e.target.value = ""; if (f) upload(f); }}
            />
          </label>
          {custom && (
            <Button variant="ghost" onClick={() => fetch("/api/bord-achtergrond", { method: "DELETE" })}><TreePine /> Terug naar hout</Button>
          )}
        </div>
      </CardContent>
    </Card>
  );
}
