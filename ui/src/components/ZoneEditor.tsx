import { useEffect, useRef, useState } from "react";
import { Circle, RotateCcw, Square, Trash } from "lucide-react";
import type { Settings, View, Zone } from "@/lib/api";
import { hash, post } from "@/lib/api";
import { cn } from "@/lib/utils";
import { Button } from "./ui/button";
import { Switch } from "./ui/switch";

const CAMERA: Zone = { round: true, x: 1075, y: -750, w: 1900, h: 1900, camera: true };
// What the editor shows: the 1920x1080 picture with room around it for zones that stick out.
const VIEW = { x: -260, y: -800, w: 3360, h: 2180 };

/** Places in the stream picture the text flows around (e.g. the round camera picture). */
export function ZoneEditor({ state }: { state: View | null }) {
  const [zones, setZones] = useState<Zone[] | null>(null);
  const [sel, setSel] = useState<number | null>(0);
  const [saved, setSaved] = useState(0);
  const svg = useRef<SVGSVGElement>(null);
  const drag = useRef<{ i: number; mode: "move" | "size"; dx: number; dy: number } | null>(null);

  useEffect(() => {
    fetch("/api/settings").then((r) => r.json()).then((s: Settings) => setZones(s.overlayZones || [CAMERA]));
  }, []);

  async function save(next: Zone[]) {
    setZones(next);
    const latest: Settings = await (await fetch("/api/settings")).json();
    await post("/api/settings", { ...latest, overlayZones: next });
    setSaved(Date.now());
  }

  const slides = state?.slides || [];
  let sample = slides.findIndex((s) => s.kind === "lezing");
  if (sample < 0) sample = slides.findIndex((s) => s.kind === "psalm");
  if (sample < 0) sample = 0;
  const thumb = slides.length ? `/api/thumb?index=${sample}&k=${hash((state?.current || "") + JSON.stringify(slides[sample]) + saved)}` : "";

  const point = (e: React.PointerEvent) => {
    const m = svg.current!.getScreenCTM()!.inverse();
    const p = new DOMPoint(e.clientX, e.clientY).matrixTransform(m);
    return { x: p.x, y: p.y };
  };
  const down = (i: number, mode: "move" | "size") => (e: React.PointerEvent) => {
    e.stopPropagation();
    (e.target as Element).setPointerCapture(e.pointerId);
    const p = point(e), z = zones![i];
    setSel(i);
    drag.current = { i, mode, dx: mode === "move" ? p.x - z.x : p.x - (z.x + z.w), dy: mode === "move" ? p.y - z.y : p.y - (z.y + z.h) };
  };
  const move = (e: React.PointerEvent) => {
    const d = drag.current;
    if (!d || !zones) return;
    const p = point(e);
    setZones(zones.map((z, k) => {
      if (k !== d.i) return z;
      if (d.mode === "move") return { ...z, x: Math.round(p.x - d.dx), y: Math.round(p.y - d.dy) };
      return { ...z, w: Math.max(60, Math.round(p.x - d.dx - z.x)), h: Math.max(60, Math.round(p.y - d.dy - z.y)) };
    }));
  };
  const up = () => {
    if (drag.current && zones) save(zones);
    drag.current = null;
  };

  if (!zones) return <div className="text-sm text-muted-foreground">Laden…</div>;
  const z = sel !== null ? zones[sel] : undefined;
  return (
    <div className="grid gap-4">
      <p className="text-sm text-muted-foreground">
        De tekst in de uitzending wijkt uit voor deze vlakken. Sleep een vlak om het te verplaatsen, en het bolletje rechtsonder om het groter of kleiner te maken.
        Het voorbeeld wordt na elke wijziging opnieuw getekend.
      </p>
      <svg
        ref={svg}
        viewBox={`${VIEW.x} ${VIEW.y} ${VIEW.w} ${VIEW.h}`}
        className="w-full touch-none rounded-xl border bg-[#0d0b09] select-none"
        onPointerMove={move}
        onPointerUp={up}
        onPointerDown={() => setSel(null)}
      >
        {thumb && <image href={thumb} x={0} y={0} width={1920} height={1080} preserveAspectRatio="none" />}
        <rect x={0} y={0} width={1920} height={1080} fill="none" stroke="#efe6d6" strokeOpacity={0.5} strokeWidth={4} />
        <text x={0} y={1140} fill="#a99b85" fontSize={44}>Beeld van de uitzending</text>
        {zones.map((q, i) => {
          const on = i === sel;
          const common = {
            fill: q.camera ? "rgb(201 164 92 / 0.10)" : "rgb(201 164 92 / 0.22)",
            stroke: "#c9a45c",
            strokeWidth: on ? 10 : 5,
            strokeDasharray: q.camera ? "30 18" : undefined,
            style: { cursor: "move" },
            onPointerDown: down(i, "move"),
          };
          return (
            <g key={i}>
              {q.round ? <ellipse cx={q.x + q.w / 2} cy={q.y + q.h / 2} rx={q.w / 2} ry={q.h / 2} {...common} /> : <rect x={q.x} y={q.y} width={q.w} height={q.h} {...common} />}
              {on && (
                <>
                  {q.round && <rect x={q.x} y={q.y} width={q.w} height={q.h} fill="none" stroke="#c9a45c" strokeOpacity={0.35} strokeWidth={3} strokeDasharray="12 12" pointerEvents="none" />}
                  <circle cx={q.x + q.w} cy={q.y + q.h} r={34} fill="#c9a45c" stroke="#1b150d" strokeWidth={6} style={{ cursor: "nwse-resize" }} onPointerDown={down(i, "size")} />
                </>
              )}
            </g>
          );
        })}
      </svg>
      <div className="flex flex-wrap items-center gap-2">
        <Button variant="outline" onClick={() => { const n = [...zones, { round: true, x: 760, y: 340, w: 400, h: 400, camera: false }]; setSel(n.length - 1); save(n); }}>
          <Circle /> Rond vlak
        </Button>
        <Button variant="outline" onClick={() => { const n = [...zones, { round: false, x: 700, y: 400, w: 520, h: 280, camera: false }]; setSel(n.length - 1); save(n); }}>
          <Square /> Rechthoek
        </Button>
        <Button variant="ghost" className="ml-auto" onClick={() => { setSel(0); save([CAMERA]); }}><RotateCcw /> Standaard herstellen</Button>
      </div>
      <div className={cn("flex flex-wrap items-center gap-4 rounded-xl border bg-background/40 px-4 py-3 transition-opacity", !z && "pointer-events-none opacity-40")}>
        <span className="text-sm font-semibold">{z ? (z.round ? "Rond vlak" : "Rechthoek") : "Kies een vlak"}</span>
        <label className="flex items-center gap-3 text-sm">
          <Switch checked={!!z?.camera} onCheckedChange={(v) => sel !== null && save(zones.map((q, k) => (k === sel ? { ...q, camera: v } : q)))} />
          Camera erdoor tonen
        </label>
        <Button variant="ghost" size="sm" className="ml-auto text-[#f3a3a8]" onClick={() => { if (sel === null) return; const n = zones.filter((_, k) => k !== sel); setSel(null); save(n); }}>
          <Trash /> Verwijderen
        </Button>
      </div>
    </div>
  );
}
