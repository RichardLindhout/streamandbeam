import { useEffect, useState } from "react";
import type { SoundInput, StreamStatus } from "@/lib/api";
import { DEVICE } from "@/lib/api";
import { sourceProblem } from "@/lib/problems";
import { cn } from "@/lib/utils";
import { Input } from "./ui/input";
import { Label } from "./ui/label";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "./ui/select";

/** Peak level -60..0 dB as 0..1. */
const fill = (db: number) => Math.min(1, Math.max(0, (db + 60) / 60));

/** How loud the sound coming in is: a bar that moves with the organ and the voice. */
export function SoundMeter({ status }: { status: StreamStatus }) {
  if (status.mode === "stop") return null;
  const level = fill(status.audioDb);
  return (
    <div
      role="meter"
      aria-label="Geluid"
      aria-valuemin={-60}
      aria-valuemax={0}
      aria-valuenow={Math.round(status.audioDb)}
      title={`Geluid ${Math.round(status.audioDb)} dB${status.audioDevice ? ` · ${status.audioDevice}` : ""}`}
      className={cn("relative h-[90px] w-4 shrink-0 overflow-hidden rounded-full bg-walnut/15 ring-2", sourceProblem(status) ? "ring-live" : "ring-walnut/30")}
    >
      <div className={cn("absolute inset-x-0 bottom-0 transition-[height] duration-150", level > 0.95 ? "bg-warn" : "bg-ok")} style={{ height: `${level * 100}%` }} />
    </div>
  );
}

const STANDARD = "standaard";
const TEST = "test";
const OWN = "eigen";

/** The setting as a choice in the list (an empty value cannot be one). */
function choice(v: string) {
  const t = v.trim();
  if (!t) return STANDARD;
  if (t === TEST) return TEST;
  if (t.startsWith(DEVICE)) return t;
  return OWN;
}

/** The same card in another USB port: Windows puts "2- " before the name. */
const plain = (n: string) => n.replace(/(^|\()\s*\d+\s*-\s+/g, "$1").toLowerCase().replace(/\s+/g, " ").trim();

function Tag({ children }: { children: React.ReactNode }) {
  return <span className="ml-3 rounded-md bg-walnut/10 px-2 py-0.5 text-[18px] font-semibold text-walnut">{children}</span>;
}

/** The sound source as a list of the inputs of the church PC (USB first), with what comes in now. */
export function SoundSourcePicker({ value, saved, onChange, status }: {
  value: string; saved?: string; onChange: (v: string) => void; status: StreamStatus;
}) {
  const [inputs, setInputs] = useState<SoundInput[] | null>(null);
  const [own, setOwn] = useState(choice(value) === OWN);
  useEffect(() => {
    let gone = false;
    const load = () =>
      fetch("/api/geluidsbronnen")
        .then((r) => r.json())
        .then((l: SoundInput[]) => !gone && setInputs(l))
        .catch(() => {});
    load();
    // A USB cable plugged in while this is open shows up by itself.
    const t = setInterval(load, 3000);
    return () => {
      gone = true;
      clearInterval(t);
    };
  }, []);

  const list = inputs || [];
  const name = value.trim().startsWith(DEVICE) ? value.trim().slice(DEVICE.length).trim() : "";
  const found = name ? list.find((i) => i.name === name) || list.find((i) => plain(i.name) === plain(name)) : undefined;
  const current = own ? OWN : found ? DEVICE + found.name : choice(value);
  const def = list.find((i) => i.default);
  const usb = list.find((i) => i.usb);
  const running = status.mode !== "stop";
  const level = fill(status.audioDb);

  function pick(v: string) {
    if (v === OWN) {
      setOwn(true);
      if (choice(value) !== OWN) onChange("");
      return;
    }
    setOwn(false);
    onChange(v === STANDARD ? "" : v);
  }

  return (
    <div className="grid gap-2">
      <Label>Geluidsbron</Label>
      <Select value={current} onValueChange={pick}>
        <SelectTrigger><SelectValue /></SelectTrigger>
        <SelectContent>
          {list.map((i) => (
            <SelectItem key={i.name} value={DEVICE + i.name}>
              {i.name}
              {i.usb && <Tag>USB</Tag>}
              {i.default && <Tag>standaard</Tag>}
            </SelectItem>
          ))}
          {name && !found && <SelectItem value={value.trim()}>{name} — niet aangesloten</SelectItem>}
          <SelectItem value={STANDARD}>Standaard van Windows{def ? ` (${def.name})` : ""}</SelectItem>
          <SelectItem value={TEST}>Testtoon</SelectItem>
          <SelectItem value={OWN}>Eigen GStreamer-regel…</SelectItem>
        </SelectContent>
      </Select>
      {current === OWN && (
        <Input aria-label="GStreamer-regel" value={value} placeholder='bv. wasapi2src device="…"' onChange={(e) => onChange(e.target.value)} />
      )}
      {inputs && !list.length && <div className="text-[18px] text-muted-foreground">Geen geluidsingangen gevonden op deze PC.</div>}
      {current === STANDARD && usb && !def?.usb && (
        <div className="rounded-xl border-2 border-warn/50 bg-warn/10 p-3 text-[18px]">
          De USB-geluidskaart <b>{usb.name}</b> is aangesloten, maar Windows neemt standaard {def ? <b>{def.name}</b> : "een andere ingang"}.
          Komt het geluid van de mixer? Kies dan de USB-geluidskaart.
        </div>
      )}
      {running && (
        <div className="grid gap-1.5 rounded-xl border-2 bg-background/50 p-3">
          <div className="flex items-baseline gap-3 text-[18px]">
            <span className="min-w-0 flex-1 truncate">Nu: <b>{status.audioDevice || "—"}</b></span>
            <span className="shrink-0 tabular-nums text-muted-foreground">{Math.round(status.audioDb)} dB</span>
          </div>
          <div className="h-3 overflow-hidden rounded-full bg-walnut/15">
            <div className={cn("h-full transition-[width] duration-150", level > 0.95 ? "bg-warn" : "bg-ok")} style={{ width: `${level * 100}%` }} />
          </div>
          {status.audioWarning && status.audioIssue !== "niet-usb" && (
            <div className={cn("text-[18px] font-semibold", sourceProblem(status) ? "text-live" : "text-[#9a5a00]")}>
              {status.audioWarning.split(" — ").slice(1).join(" — ") || status.audioWarning}
            </div>
          )}
          {saved !== undefined && value.trim() !== saved.trim() && (
            <div className="text-[18px] text-muted-foreground">Na Opslaan neemt de uitzending de nieuwe geluidsbron meteen over.</div>
          )}
        </div>
      )}
    </div>
  );
}
