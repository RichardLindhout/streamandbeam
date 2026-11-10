import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import Moveable from "react-moveable";
import { ArrowDown, ArrowUp, Camera, Copy, Eye, EyeOff, Image as ImageIcon, Plus, QrCode, Square, Trash, Type, Upload } from "lucide-react";
import { toast } from "sonner";
import type { BoxKind, Layout, LayoutBox, StreamStatus, Target, View } from "@/lib/api";
import { at, errorText, hhmm, post } from "@/lib/api";
import { FIELDS, KIND_LABEL, MOMENTS, SIZE, STARTS, box, sample } from "@/lib/layouts";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { Confirm } from "@/components/Confirm";

const TARGETS: { id: Target; label: string }[] = [
  { id: "uitzending", label: "Uitzending" },
  { id: "bord", label: "Borden in de kerk" },
];

const KIND_ICON: Record<BoxKind, React.ReactNode> = {
  tekst: <Type />, afbeelding: <ImageIcon />, vlak: <Square />, camera: <Camera />, qr: <QrCode />,
};

const select = "h-14 w-full rounded-xl border-2 border-input bg-card px-3 text-[20px]";
const momentLabel = (target: Target, id: string) => MOMENTS[target].find((m) => m.id === id)?.label ?? id;

/** A screen drawn by the same code as the TVs (slide.js), small, for the list. */
function Mini({ layout }: { layout: Layout }) {
  const canvas = useRef<HTMLDivElement>(null);
  const fields = useMemo(() => sample(layout.target, layout.moment), [layout.target, layout.moment]);
  useLayoutEffect(() => {
    if (canvas.current) renderLayout(canvas.current, layout, fields);
  });
  const { w, h } = SIZE[layout.target];
  return (
    <div className="mon pointer-events-none shrink-0 rounded-md" style={{ width: layout.target === "bord" ? 54 : 128, aspectRatio: `${w} / ${h}` }}>
      <div className="canvas" ref={canvas} />
    </div>
  );
}

/** The picture with its boxes: tap a box to choose it, then drag it or pull a corner. */
function Stage({ draft, fields, sel, onSelect, onMove }: {
  draft: Layout; fields: Record<string, string>; sel: string | null;
  onSelect: (id: string | null) => void; onMove: (id: string, at: Pick<LayoutBox, "x" | "y" | "w" | "h">) => void;
}) {
  const frame = useRef<HTMLDivElement>(null);
  const canvas = useRef<HTMLDivElement>(null);
  const moveable = useRef<Moveable>(null);
  const [el, setEl] = useState<HTMLElement | null>(null);
  const { w, h } = SIZE[draft.target];

  const draw = () => {
    if (!canvas.current) return;
    renderLayout(canvas.current, draft, fields);
    setEl(sel ? canvas.current.querySelector<HTMLElement>(`[data-box="${CSS.escape(sel)}"]`) : null);
  };
  useLayoutEffect(draw, [draft, fields, sel]); // eslint-disable-line react-hooks/exhaustive-deps
  // Drawn again when the frame changes size. Once: drawing replaces the boxes, so an observer
  // per render would take the box away from under a drag.
  const latest = useRef(draw);
  latest.current = draw;
  useEffect(() => {
    if (!frame.current) return;
    let first = true;
    const ro = new ResizeObserver(() => {
      if (first) return void (first = false);
      latest.current();
    });
    ro.observe(frame.current);
    return () => ro.disconnect();
  }, []);
  useLayoutEffect(() => moveable.current?.updateRect(), [el]);

  const done = (t: HTMLElement | SVGElement) => {
    const s = (t as HTMLElement).style, px = (v: string) => Math.round(parseFloat(v) || 0);
    if (sel) onMove(sel, { x: px(s.left), y: px(s.top), w: Math.max(10, px(s.width)), h: Math.max(10, px(s.height)) });
  };

  return (
    <div className="relative mx-auto" style={{ aspectRatio: `${w} / ${h}`, maxHeight: "100%", maxWidth: "100%", height: draft.target === "bord" ? "100%" : undefined, width: draft.target === "bord" ? undefined : "100%" }}>
      <div
        ref={frame}
        className="mon h-full w-full rounded-lg ring-2 ring-walnut/40"
        onPointerDown={(e) => onSelect((e.target as HTMLElement).closest<HTMLElement>("[data-box]")?.dataset.box ?? null)}
      >
        <div className="canvas" ref={canvas} />
      </div>
      {el && (
        <Moveable
          ref={moveable}
          target={el}
          draggable
          resizable
          origin={false}
          throttleDrag={1}
          throttleResize={1}
          onDrag={(e) => {
            e.target.style.left = `${e.left}px`;
            e.target.style.top = `${e.top}px`;
          }}
          onDragEnd={(e) => e.isDrag && done(e.target)}
          onResize={(e) => {
            e.target.style.width = `${e.width}px`;
            e.target.style.height = `${e.height}px`;
            e.target.style.left = `${e.drag.left}px`;
            e.target.style.top = `${e.drag.top}px`;
          }}
          onResizeEnd={(e) => e.isDrag && done(e.target)}
        />
      )}
    </div>
  );
}

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <label className="grid gap-1 text-[18px] text-muted-foreground">
      {label}
      <div className="flex items-center gap-2 text-foreground">{children}</div>
    </label>
  );
}

function Num({ value, onChange, min, max, step }: { value: number; onChange: (n: number) => void; min?: number; max?: number; step?: number }) {
  return (
    <Input
      type="number" inputMode="numeric" min={min} max={max} step={step} value={Number.isFinite(value) ? value : 0}
      onChange={(e) => e.target.value !== "" && onChange(+e.target.value)}
      className="h-14 min-w-0 px-3 text-[20px] tabular-nums"
    />
  );
}

function Colour({ value, onChange }: { value: string; onChange: (c: string) => void }) {
  return <input type="color" value={/^#[0-9a-f]{6}$/i.test(value) ? value : "#000000"} onChange={(e) => onChange(e.target.value)} className="h-14 w-20 shrink-0 cursor-pointer rounded-xl border-2 border-input bg-card p-1" />;
}

function Choice<T extends string>({ value, options, onChange }: { value: T; options: [T, string][]; onChange: (v: T) => void }) {
  return (
    <div className="flex w-full rounded-xl border-2 border-input bg-card p-1">
      {options.map(([v, label]) => (
        <button key={v} type="button" onClick={() => onChange(v)} className={cn("h-11 flex-1 rounded-lg text-[18px] font-semibold", value === v ? "bg-walnut text-white" : "text-muted-foreground hover:bg-accent")}>
          {label}
        </button>
      ))}
    </div>
  );
}

function Check({ checked, onChange, children }: { checked: boolean; onChange: (b: boolean) => void; children: React.ReactNode }) {
  return (
    <label className="flex h-12 cursor-pointer items-center gap-3 text-[20px]">
      <input type="checkbox" checked={checked} onChange={(e) => onChange(e.target.checked)} className="size-6 accent-[var(--walnut,#5a3e2b)]" />
      {children}
    </label>
  );
}

/** Choose one of the church's pictures, or add a new one. */
export function Picture({ value, onChange }: { value: string; onChange: (name: string) => void }) {
  const [names, setNames] = useState<string[]>([]);
  const file = useRef<HTMLInputElement>(null);
  const load = () => fetch("/api/afbeeldingen").then((r) => r.json()).then(setNames).catch(() => {});
  useEffect(() => { load(); }, []);
  async function upload(f?: File) {
    if (!f) return;
    const r = await fetch("/api/afbeelding?naam=" + encodeURIComponent(f.name), { method: "POST", headers: { "Content-Type": f.type }, body: f }).catch(() => null);
    if (!r?.ok) return void toast.error(r ? await errorText(r) : "Geen verbinding met de kerk-PC");
    const { name } = await r.json();
    await load();
    onChange(name);
  }
  return (
    <>
      <select className={select} value={value} onChange={(e) => onChange(e.target.value)}>
        <option value="">Kies een afbeelding…</option>
        {names.map((n) => <option key={n} value={n}>{n}</option>)}
      </select>
      <Button variant="outline" size="icon-sm" aria-label="Afbeelding toevoegen" className="shrink-0" onClick={() => file.current?.click()}><Upload /></Button>
      <input ref={file} type="file" accept="image/png,image/jpeg" hidden onChange={(e) => { upload(e.target.files?.[0]); e.target.value = ""; }} />
    </>
  );
}

/** Everything of one box: place, text, colours. */
function BoxFields({ b, set }: { b: LayoutBox; set: (patch: Partial<LayoutBox>) => void }) {
  const text = useRef<HTMLTextAreaElement>(null);
  const insert = (field: string) => {
    const t = text.current, at = t?.selectionStart ?? b.text.length, end = t?.selectionEnd ?? at;
    set({ text: b.text.slice(0, at) + `{${field}}` + b.text.slice(end) });
  };
  return (
    <div className="grid gap-3">
      {b.kind === "tekst" && (
        <>
          <Textarea ref={text} rows={3} value={b.text} onChange={(e) => set({ text: e.target.value })} className="text-[20px]" />
          <div className="flex flex-wrap gap-1.5">
            {FIELDS.map((f) => (
              <button key={f.id} type="button" onClick={() => insert(f.id)} className="rounded-lg border-2 border-input bg-card px-2.5 py-1 text-[17px] hover:bg-accent">
                + {f.label}
              </button>
            ))}
          </div>
          <div className="grid grid-cols-2 gap-3">
            <Row label="Letter">
              <select className={select} value={b.font} onChange={(e) => set({ font: e.target.value as LayoutBox["font"] })}>
                <option value="sans">Schreefloos</option>
                <option value="serif">Schreef</option>
              </select>
            </Row>
            <Row label="Grootte (hoogstens)"><Num value={b.size} min={14} max={500} onChange={(size) => set({ size })} /></Row>
          </div>
          <div className="flex items-center gap-4">
            <Colour value={b.color} onChange={(color) => set({ color })} />
            <Check checked={b.bold} onChange={(bold) => set({ bold })}>Vet</Check>
          </div>
          <Choice value={b.align} options={[["links", "Links"], ["midden", "Midden"], ["rechts", "Rechts"]]} onChange={(align) => set({ align })} />
          <Choice value={b.valign} options={[["boven", "Boven"], ["midden", "Midden"], ["onder", "Onder"]]} onChange={(valign) => set({ valign })} />
          <Check checked={!!b.fill} onChange={(on) => set({ fill: on ? "#005e70" : "" })}>Gekleurd vlak achter de tekst</Check>
        </>
      )}
      {b.kind === "afbeelding" && (
        <>
          <Row label="Afbeelding (JPG of PNG)"><Picture value={b.image} onChange={(image) => set({ image })} /></Row>
          <Choice value={b.fit} options={[["vullen", "Vak vullen"], ["passend", "Hele afbeelding"]]} onChange={(fit) => set({ fit })} />
        </>
      )}
      {(b.kind === "vlak" || (b.kind === "tekst" && !!b.fill)) && (
        <div className="grid grid-cols-[auto_1fr] items-end gap-3">
          <Row label="Kleur"><Colour value={b.fill} onChange={(fill) => set({ fill })} /></Row>
          <Row label="Dekking (%)"><Num value={Math.round(b.opacity * 100)} min={0} max={100} step={5} onChange={(n) => set({ opacity: Math.max(0, Math.min(100, n)) / 100 })} /></Row>
        </div>
      )}
      {(b.kind === "vlak" || b.kind === "camera") && (
        <div className="grid grid-cols-2 items-end gap-3">
          <Check checked={b.round} onChange={(round) => set({ round })}>Rond</Check>
          {!b.round && <Row label="Ronde hoeken"><Num value={b.radius} min={0} max={540} onChange={(radius) => set({ radius })} /></Row>}
        </div>
      )}
      {b.kind === "camera" && <p className="text-[18px] leading-snug text-muted-foreground">De camera vult dit vak; wat buiten het vak valt, is niet te zien. Vakken die erna komen, liggen over de camera.</p>}
      {b.kind === "qr" && <p className="text-[18px] leading-snug text-muted-foreground">De QR-code van de link bij de instellingen.</p>}
      <div className="grid grid-cols-4 gap-2">
        <Row label="Links"><Num value={b.x} onChange={(x) => set({ x })} /></Row>
        <Row label="Boven"><Num value={b.y} onChange={(y) => set({ y })} /></Row>
        <Row label="Breed"><Num value={b.w} min={10} onChange={(w) => set({ w: Math.max(10, w) })} /></Row>
        <Row label="Hoog"><Num value={b.h} min={10} onChange={(h) => set({ h: Math.max(10, h) })} /></Row>
      </div>
    </div>
  );
}

/** The stream picture as the program itself draws it (the letters can break a little differently
 *  than in the browser). */
function RealPreview({ draft, fields }: { draft: Layout; fields: Record<string, string> }) {
  const [url, setUrl] = useState("");
  useEffect(() => {
    let live = true;
    const t = setTimeout(async () => {
      const r = await fetch("/api/scherm-voorbeeld", { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify({ layout: draft, fields }) }).catch(() => null);
      if (!r?.ok || !live) return;
      const next = URL.createObjectURL(await r.blob());
      setUrl((old) => {
        if (old) URL.revokeObjectURL(old);
        return next;
      });
    }, 500);
    return () => {
      live = false;
      clearTimeout(t);
    };
  }, [draft, fields]);
  if (!url) return null;
  return (
    <div className="grid gap-1">
      <div className="text-[18px] text-muted-foreground">Zo komt het in de uitzending</div>
      <img src={url} alt="" className="w-full rounded-md ring-1 ring-walnut/30" />
    </div>
  );
}

export function Schermen({ state, status, onBack }: { state: View | null; status: StreamStatus; onBack: () => void }) {
  const [target, setTarget] = useState<Target>("uitzending");
  const [draft, setDraft] = useState<Layout | null>(null);
  const [dirty, setDirty] = useState(false);
  const [sel, setSel] = useState<string | null>(null);
  const [ask, setAsk] = useState<null | { title: string; yes: string; run: () => void }>(null);
  const live = status.mode === "live";
  const all = state?.layouts || [];
  const list = all.filter((l) => l.target === target);
  const fields = useMemo(() => (draft ? sample(draft.target, draft.moment) : {}), [draft?.target, draft?.moment]); // eslint-disable-line react-hooks/exhaustive-deps
  const chosen = draft?.boxes.find((b) => b.id === sel);
  const shown = !!draft?.id && (draft.target === "bord" ? state?.boardLayout : state?.streamLayout) === draft.id;

  /** Runs `run` at once, or first asks to throw the unsaved changes away. */
  const leave = (run: () => void) => (dirty ? setAsk({ title: "Wijzigingen weggooien?", yes: "Weggooien", run }) : run());
  const open = (l: Layout | null) => {
    setDraft(l ? structuredClone(l) : null);
    setDirty(false);
    setSel(null);
  };
  const change = (patch: Partial<Layout>) => {
    setDraft((d) => (d ? { ...d, ...patch } : d));
    setDirty(true);
  };
  const setBox = (id: string, patch: Partial<LayoutBox>) => draft && change({ boxes: draft.boxes.map((b) => (b.id === id ? { ...b, ...patch } : b)) });
  const add = (kind: BoxKind) => {
    if (!draft) return;
    const { w, h } = SIZE[draft.target];
    const b = box(kind, kind === "camera" ? { x: 0, y: 0, w, h } : kind === "qr" ? { w: 200, h: 200 } : { x: Math.round(w * 0.1), y: Math.round(h * 0.1), w: Math.round(w * 0.5), h: kind === "tekst" ? 160 : Math.round(h * 0.3) });
    // The camera goes at the back, so the other boxes lie over it.
    change({ boxes: kind === "camera" ? [b, ...draft.boxes] : [...draft.boxes, b] });
    setSel(b.id);
  };
  const shift = (id: string, by: number) => {
    if (!draft) return;
    const i = draft.boxes.findIndex((b) => b.id === id), j = i + by;
    if (i < 0 || j < 0 || j >= draft.boxes.length) return;
    const boxes = [...draft.boxes];
    [boxes[i], boxes[j]] = [boxes[j], boxes[i]];
    change({ boxes });
  };
  const remove = (id: string) => {
    if (!draft) return;
    change({ boxes: draft.boxes.filter((b) => b.id !== id) });
    setSel(null);
  };

  async function save() {
    if (!draft) return;
    const r = await post("/api/scherm", draft);
    if (!r.ok) return;
    // The chosen box stays chosen (a new box keeps its id).
    setDraft(await r.json());
    setDirty(false);
    toast.success("Opgeslagen");
  }
  async function destroy() {
    if (!draft) return;
    if (draft.id && !(await post("/api/scherm-weg?id=" + encodeURIComponent(draft.id))).ok) return;
    open(null);
  }

  // Delete and the arrow keys work on the chosen box, unless the cursor is in a field.
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (!chosen || (e.target as HTMLElement | null)?.closest?.("input, textarea, select")) return;
      const step = e.shiftKey ? 10 : 1;
      const move: Record<string, Partial<LayoutBox>> = {
        ArrowLeft: { x: chosen.x - step }, ArrowRight: { x: chosen.x + step }, ArrowUp: { y: chosen.y - step }, ArrowDown: { y: chosen.y + step },
      };
      if (e.key === "Delete" || e.key === "Backspace") remove(chosen.id);
      else if (move[e.key]) setBox(chosen.id, move[e.key]);
      else return;
      e.preventDefault();
    };
    document.addEventListener("keydown", key);
    return () => document.removeEventListener("keydown", key);
  });

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex h-24 shrink-0 items-center gap-5 border-b-2 border-walnut/15 bg-[#efe5cc] px-4">
        <Button variant="outline" onClick={() => leave(onBack)} className="text-[24px]">← Terug naar de dienst</Button>
        <h1 className="min-w-0 flex-1 truncate font-serif text-[30px] font-semibold">Schermen en beelden</h1>
        {live && (
          <span className="flex h-14 shrink-0 items-center gap-2.5 rounded-full bg-live px-5 text-[22px] font-bold text-white tabular-nums">
            <span className="size-3 animate-pulse-dot rounded-full bg-white" /> LIVE {hhmm(at(status.since))}
          </span>
        )}
        <div role="radiogroup" aria-label="Soort" className="flex shrink-0 rounded-2xl border-2 border-input bg-card p-1">
          {TARGETS.map((t) => (
            <button
              key={t.id} role="radio" aria-checked={target === t.id}
              onClick={() => target !== t.id && leave(() => { setTarget(t.id); open(null); })}
              className={cn("h-14 rounded-xl px-6 text-[22px] font-semibold transition", target === t.id ? "bg-walnut text-white" : "text-muted-foreground hover:bg-accent")}
            >
              {t.label}
            </button>
          ))}
        </div>
      </div>

      <div className="flex min-h-0 flex-1">
        {/* The screens of this kind. */}
        <aside className="scroll-thin flex w-[360px] shrink-0 flex-col gap-2 overflow-y-auto border-r-2 border-walnut/15 bg-[#efe5cc]/60 p-3">
          <DropdownMenu modal={false}>
            <DropdownMenuTrigger asChild>
              <Button size="lg" className="w-full"><Plus /> Nieuw</Button>
            </DropdownMenuTrigger>
            <DropdownMenuContent className="w-[22rem]">
              {STARTS[target].map((s) => (
                <DropdownMenuItem key={s.label} onSelect={() => leave(() => { setDraft({ id: "", ...s.make() }); setDirty(true); setSel(null); })}>{s.label}</DropdownMenuItem>
              ))}
            </DropdownMenuContent>
          </DropdownMenu>
          {list.map((l) => (
            <button
              key={l.id}
              onClick={() => draft?.id !== l.id && leave(() => open(l))}
              className={cn("flex items-center gap-3 rounded-xl border-2 p-2 text-left transition", draft?.id === l.id ? "border-primary bg-[#fdf8ec]" : "border-transparent hover:bg-accent")}
            >
              <Mini layout={l} />
              <span className="min-w-0 flex-1">
                <span className="block truncate text-[21px] font-semibold">{l.name || "Naamloos"}</span>
                <span className="block truncate text-[17px] text-muted-foreground">{momentLabel(target, l.moment)}</span>
              </span>
            </button>
          ))}
          {!list.length && (
            <p className="px-2 py-4 text-[20px] leading-snug text-muted-foreground">
              Nog geen eigen {target === "bord" ? "borden" : "beelden"}. Alles is automatisch. Met Nieuw maak je er een.
            </p>
          )}
        </aside>

        {draft ? (
          <>
            <main className="flex min-w-0 flex-1 flex-col gap-3 p-4">
              <div className="flex shrink-0 flex-wrap items-center gap-2">
                {(["tekst", "vlak", "afbeelding", ...(draft.target === "uitzending" ? ["camera"] : []), "qr"] as BoxKind[]).map((k) => (
                  <Button key={k} variant="outline" size="sm" disabled={k === "camera" && draft.boxes.some((b) => b.kind === "camera")} onClick={() => add(k)}>
                    {KIND_ICON[k]} {KIND_LABEL[k]}
                  </Button>
                ))}
                <span className="flex-1" />
                {draft.id && !dirty && (
                  <Button variant="outline" size="sm" onClick={() => post("/api/scherm-tonen", { id: draft.id, aan: !shown })}>
                    {shown ? <><EyeOff /> Weghalen</> : <><Eye /> Nu tonen</>}
                  </Button>
                )}
                <Button variant="outline" size="icon-sm" aria-label="Kopie maken" onClick={() => leave(() => { setDraft({ ...structuredClone(draft), id: "", moment: "", name: `${draft.name} (kopie)` }); setDirty(true); })}><Copy /></Button>
                <Button variant="outline" size="icon-sm" aria-label="Verwijderen" className="text-live" onClick={() => setAsk({ title: `${draft.name || "Dit scherm"} verwijderen?`, yes: "Verwijderen", run: destroy })}><Trash /></Button>
                <Button size="sm" disabled={!dirty} onClick={save} className="px-8">Opslaan</Button>
              </div>
              <div className="flex min-h-0 flex-1 items-center justify-center">
                <Stage draft={draft} fields={fields} sel={sel} onSelect={setSel} onMove={setBox} />
              </div>
            </main>

            <aside className="scroll-thin grid w-[440px] shrink-0 content-start gap-5 overflow-y-auto border-l-2 border-walnut/15 bg-[#efe5cc]/60 p-4">
              <div className="grid gap-3">
                <Row label="Naam"><Input value={draft.name} onChange={(e) => change({ name: e.target.value })} className="h-14 text-[20px]" /></Row>
                <Row label="Vervangt het automatische beeld bij">
                  <select className={select} value={draft.moment} onChange={(e) => change({ moment: e.target.value })}>
                    {MOMENTS[draft.target].map((m) => <option key={m.id} value={m.id}>{m.label}</option>)}
                  </select>
                </Row>
                <div className="flex items-end gap-4">
                  <Row label="Achtergrond"><Colour value={draft.background || "#000000"} onChange={(background) => change({ background })} /></Row>
                  {draft.target === "bord" && <Check checked={!draft.background} onChange={(on) => change({ background: on ? "" : "#000000" })}>Achtergrond van het bord</Check>}
                </div>
              </div>

              <div className="grid gap-1">
                <div className="hymnal text-[20px] text-[#7a5a1c]">vakken (voorste bovenaan)</div>
                {[...draft.boxes].reverse().map((b) => (
                  <div key={b.id} className={cn("flex items-center rounded-xl border-2", sel === b.id ? "border-primary bg-[#fdf8ec]" : "border-transparent")}>
                    <button onClick={() => setSel(b.id)} className="flex h-12 min-w-0 flex-1 items-center gap-2 px-2 text-left text-[19px] [&_svg]:size-5 [&_svg]:shrink-0">
                      {KIND_ICON[b.kind]} <span className="truncate">{b.kind === "tekst" ? b.text.replace(/\n/g, " ") || "Tekst" : b.kind === "afbeelding" ? b.image || "Afbeelding" : KIND_LABEL[b.kind]}</span>
                    </button>
                    <button aria-label="Naar voren" onClick={() => shift(b.id, 1)} className="grid size-11 place-items-center rounded-lg hover:bg-accent"><ArrowUp className="size-5" /></button>
                    <button aria-label="Naar achteren" onClick={() => shift(b.id, -1)} className="grid size-11 place-items-center rounded-lg hover:bg-accent"><ArrowDown className="size-5" /></button>
                    <button aria-label="Vak weghalen" onClick={() => remove(b.id)} className="grid size-11 place-items-center rounded-lg text-live hover:bg-accent"><Trash className="size-5" /></button>
                  </div>
                ))}
                {!draft.boxes.length && <p className="text-[18px] text-muted-foreground">Nog geen vakken. Kies er een boven het beeld.</p>}
              </div>

              {chosen && (
                <div className="grid gap-3">
                  <div className="hymnal text-[20px] text-[#7a5a1c]">{KIND_LABEL[chosen.kind]}</div>
                  <BoxFields key={chosen.id} b={chosen} set={(patch) => setBox(chosen.id, patch)} />
                </div>
              )}
              {draft.target === "uitzending" && <RealPreview draft={draft} fields={fields} />}
            </aside>
          </>
        ) : (
          <div className="grid flex-1 place-items-center p-8 text-center text-[26px] text-muted-foreground">Kies links een scherm, of maak een nieuw.</div>
        )}
      </div>

      <Confirm
        open={!!ask}
        onOpenChange={(o) => !o && setAsk(null)}
        title={ask?.title ?? ""}
        yes={ask?.yes ?? "Ja"}
        tone="live"
        onYes={() => {
          const run = ask?.run;
          setAsk(null);
          setDirty(false);
          run?.();
        }}
      />
    </div>
  );
}
