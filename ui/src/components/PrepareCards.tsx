import { useEffect, useRef, useState } from "react";
import { Circle, Eye, ImageUp, Radio, Square, TreePine } from "lucide-react";
import { toast } from "sonner";
import type { Mode, Service, StreamStatus, View } from "@/lib/api";
import { errorText, quick, saveService, setStreamMode } from "@/lib/api";
import { cn } from "@/lib/utils";
import { Card, CardContent, CardHeader } from "./ui/card";
import { Button, buttonVariants } from "./ui/button";
import { Label } from "./ui/label";
import { BlurField } from "./BlurField";
import { Board } from "./Board";
import { Confirm } from "./Confirm";

/** A field that saves when you leave it and then says so for a few seconds. */
function SavingField({ label, value, multiline, rows, onSave }: {
  label: string; value: string; multiline?: boolean; rows?: number; onSave: (v: string) => Promise<boolean>;
}) {
  const [state, setState] = useState<"" | "ok" | "bad">("");
  const t = useRef(0);
  useEffect(() => () => clearTimeout(t.current), []);
  return (
    <div className="grid gap-2">
      <div className="flex items-baseline gap-3">
        <Label>{label}</Label>
        {state === "ok" && <span className="text-[18px] font-semibold text-ok">✓ Opgeslagen</span>}
        {state === "bad" && <span className="text-[18px] font-semibold text-live">Niet opgeslagen</span>}
      </div>
      <BlurField
        multiline={multiline}
        rows={rows}
        value={value}
        onSave={async (v) => {
          const ok = await onSave(v);
          setState(ok ? "ok" : "bad");
          clearTimeout(t.current);
          t.current = window.setTimeout(() => setState(""), 4000);
        }}
      />
    </div>
  );
}

export function SermonCard({ svc }: { svc: Service }) {
  const latest = useRef(svc);
  latest.current = svc;
  const save = async (patch: Partial<Service>) => (await saveService({ ...latest.current, ...patch })).ok;
  return (
    <Card>
      <CardHeader>Preek</CardHeader>
      <CardContent className="grid gap-5">
        <SavingField label="Predikant" value={svc.predikant} onSave={(v) => save({ predikant: v })} />
        <SavingField
          label="Tekst"
          value={svc.text?.ref || ""}
          onSave={async (v) => {
            try {
              const [it] = await quick(v);
              return it ? save({ text: it }) : false;
            } catch {
              return false;
            }
          }}
        />
        <SavingField label="Thema" value={svc.theme} onSave={(v) => save({ theme: v })} />
        <SavingField
          label="Punten"
          multiline
          rows={3}
          value={(svc.points || []).join("\n")}
          onSave={(v) => save({ points: v.split("\n").map((s) => s.trim()).filter(Boolean) })}
        />
      </CardContent>
    </Card>
  );
}

/** Voorbeeld, Opnemen, Live and Stop, for the technician. Hidden while live. */
export function StreamControls({ status }: { status: StreamStatus }) {
  const [ask, setAsk] = useState(false);
  if (status.mode === "live") return <div className="text-[24px] font-semibold">Live: eerst stoppen</div>;
  const modes: [Mode, string, React.ReactNode][] = [
    ["voorbeeld", "Voorbeeld", <Eye key="v" />],
    ["opnemen", "Opnemen", <Circle key="o" className="fill-current text-live" />],
    ["live", "Live", <Radio key="l" />],
    ["stop", "Stop", <Square key="s" className="fill-current" />],
  ];
  return (
    <div className="grid gap-4">
      <div className="grid grid-cols-2 gap-3">
        {modes.map(([m, label, icon]) => (
          <Button
            key={m}
            variant="outline"
            onClick={() => (m === "live" ? setAsk(true) : setStreamMode(m))}
            className={cn(status.mode === m && "border-walnut bg-[#efe3c6]")}
          >
            {icon} {label}
          </Button>
        ))}
      </div>
      {status.recording && <div className="truncate text-[18px] text-muted-foreground">Opname: {status.recording.split(/[\\/]/).pop()}</div>}
      {status.error && <div className="rounded-xl border-2 border-warn/50 bg-warn/10 p-3 text-[18px]">{status.error}</div>}
      <Confirm open={ask} onOpenChange={setAsk} title="Live gaan?" yes="Ja, live" tone="live" onYes={() => setStreamMode("live")} />
    </div>
  );
}

/** The background picture of the boards in church: the wood, or a photo of the church's own. */
export function BoardBackground({ state }: { state: View | null }) {
  const [busy, setBusy] = useState(false);
  const sample = state?.boards?.find((b) => b.kind === "bord") || { kind: "bord", item: 0, header: "", sub: "", list: ["Ps 42:1", "Ps 119:5", "Joh. 3:16", "", "Ps 25:2"] };
  const custom = (state?.boardBackground ?? 0) > 0;
  async function upload(file: File) {
    setBusy(true);
    try {
      const r = await fetch("/api/bord-achtergrond", { method: "POST", headers: { "Content-Type": file.type }, body: file });
      if (!r.ok) toast.error(await errorText(r));
      else toast.success("Nieuwe achtergrond");
    } catch {
      toast.error("Geen verbinding met de kerk-PC");
    }
    setBusy(false);
  }
  async function reset() {
    try {
      const r = await fetch("/api/bord-achtergrond", { method: "DELETE" });
      if (!r.ok) toast.error(await errorText(r));
    } catch {
      toast.error("Geen verbinding met de kerk-PC");
    }
  }
  return (
    <div className="grid grid-cols-[150px_minmax(0,1fr)] items-start gap-6">
      <Board slide={sample} className="rounded-lg ring-2 ring-walnut/30" />
      <div className="grid gap-3">
        <div className="text-[20px]">{custom ? "Eigen afbeelding" : "Hout"}</div>
        <label className={cn(buttonVariants({ variant: "outline" }), "cursor-pointer", busy && "pointer-events-none opacity-50")}>
          <ImageUp /> {busy ? "Bezig…" : "Kies afbeelding…"}
          <input
            type="file"
            accept="image/jpeg,image/png,image/webp"
            className="sr-only"
            onChange={(e) => { const f = e.target.files?.[0]; e.target.value = ""; if (f) upload(f); }}
          />
        </label>
        {custom && <Button variant="ghost" onClick={reset}><TreePine /> Terug naar hout</Button>}
      </div>
    </div>
  );
}
