import { useRef, useState } from "react";
import { ArrowDown, ArrowUp, GripVertical, LoaderCircle, Lock, Plus, X } from "lucide-react";
import { toast } from "sonner";
import type { Item, Service } from "@/lib/api";
import { quick, saveService } from "@/lib/api";
import { itemDetail, itemLabel } from "@/lib/kinds";
import { cn } from "@/lib/utils";
import { Button } from "./ui/button";
import { Input } from "./ui/input";

/** Startscherm and Aftiteling stay at the top and the bottom. */
const LOCKED = new Set(["welkom", "einde"]);
const PSALMS = new Set(["psalm", "slotzang", "nazang"]);
/** Parts whose psalm or text can be swapped by typing a new one. */
const CHANGEABLE = new Set(["psalm", "slotzang", "nazang", "lezing", "catechismus", "belijdenis"]);

/** "1-3, 5" → [1, 2, 3, 5] */
function verses(s: string) {
  const out: number[] = [];
  for (const part of s.split(/[,;\s]+/).filter(Boolean)) {
    const [a, b] = part.split("-").map((x) => parseInt(x, 10));
    if (!a) return null;
    for (let v = a; v <= (b && b >= a && b - a < 40 ? b : a); v++) out.push(v);
  }
  return out.length ? out : null;
}

/** "84:3", "ps 84 : 3,5", "Psalm 84:1-3" → psalm and verses, without asking Claude. */
export function parsePsalm(s: string): { psalm: number; verses: number[] } | null {
  const m = s.trim().match(/^(?:ps(?:alm)?\.?\s*)?(\d{1,3})\s*:\s*([\d\s,;-]+)$/i);
  if (!m) return null;
  const psalm = parseInt(m[1], 10);
  const vs = verses(m[2]);
  return psalm >= 1 && psalm <= 150 && vs ? { psalm, verses: vs } : null;
}

/** What the change field starts with: "Ps 42:5", "Johannes 3 : 16". */
function prefill(it: Item) {
  if (PSALMS.has(it.kind) && Number(it.psalm) > 0 && Number(it.psalm) <= 150) return `Ps ${it.psalm}:${((it.verses as number[]) || []).join(",")}`;
  if (it.kind === "catechismus") return `HC ${it.ref}`;
  return it.ref || "";
}

/** Where typed parts go when no row is chosen: before the blessing and the end card. */
function defaultSpot(items: Item[]) {
  const zegen = items.findIndex((i) => i.kind === "zegen");
  if (zegen >= 0) return zegen;
  let at = items.findIndex((i) => i.kind === "einde");
  if (at < 0) at = items.length;
  if (at > 0 && items[at - 1]?.kind === "zegen") at--;
  return at;
}

type Drag = { from: number; y0: number; s0: number; tops: number[]; hs: number[]; dy: number; to: number };

/** The order in edit mode: drag rows (or ↑/↓), ✕ to remove, type to add, tap a psalm or reading to change it. */
export function OrderEdit({ svc, cur }: { svc: Service; cur: number }) {
  const latest = useRef(svc);
  latest.current = svc;
  const box = useRef<HTMLDivElement>(null);
  const rows = useRef<(HTMLLIElement | null)[]>([]);
  const [sel, setSel] = useState<number | null>(null);
  const [change, setChange] = useState<{ i: number; text: string; busy?: boolean; error?: string } | null>(null);
  const [typed, setTyped] = useState("");
  const [busy, setBusy] = useState<{ text: string; spin?: boolean; bad?: boolean } | null>(null);
  const [drag, setDrag] = useState<Drag | null>(null);

  const items = svc.items;
  const lead = items.findIndex((i) => !LOCKED.has(i.kind)) < 0 ? items.length : items.findIndex((i) => !LOCKED.has(i.kind));
  let tail = 0;
  while (tail < items.length - lead && LOCKED.has(items[items.length - 1 - tail].kind)) tail++;
  const update = (list: Item[]) => saveService({ ...latest.current, items: list });

  function moveTo(from: number, to: number) {
    const list = [...latest.current.items];
    if (to === from || to < lead || to > list.length - 1 - tail) return;
    const [it] = list.splice(from, 1);
    list.splice(to, 0, it);
    update(list);
    if (sel === from) setSel(to);
    setChange(null);
  }

  function remove(i: number) {
    const list = [...latest.current.items];
    const [gone] = list.splice(i, 1);
    update(list);
    setSel(null);
    setChange(null);
    toast(`${itemLabel(gone)} weggehaald`, {
      duration: 10000,
      action: {
        label: "Ongedaan maken",
        onClick: () => {
          const l = [...latest.current.items];
          l.splice(Math.min(i, l.length), 0, gone);
          update(l);
        },
      },
    });
  }

  const spot = sel !== null && items[sel] ? Math.max(lead, Math.min(sel + 1, items.length - tail)) : defaultSpot(items);

  async function add() {
    if (!typed.trim()) return;
    setBusy({ text: "Bezig…", spin: true });
    let added: Item[];
    try {
      added = (await quick(typed)).map((it, k) => ({ ...it, id: "i" + Date.now() + k }));
    } catch (e) {
      return setBusy({ text: (e as Error).message, bad: true });
    }
    const list = [...latest.current.items];
    list.splice(spot, 0, ...added);
    update(list);
    setSel(spot + added.length - 1);
    const unsure = added.filter((i) => i.problem).length;
    setBusy({ text: `${added.length} toegevoegd` + (unsure ? ` · ${unsure} nakijken` : ""), bad: unsure > 0 });
    setTyped("");
  }

  async function replace() {
    if (!change) return;
    const i = change.i;
    const it = latest.current.items[i];
    if (!it || !change.text.trim()) return;
    let next: Item | null = null;
    const ps = PSALMS.has(it.kind) ? parsePsalm(change.text) : null;
    if (ps) next = { ...it, ...ps, problem: "", custom: [] };
    else {
      setChange({ ...change, busy: true, error: "" });
      try {
        const [got] = await quick(change.text);
        if (!got) throw new Error("Niet herkend");
        const same = got.kind === it.kind || (PSALMS.has(got.kind) && PSALMS.has(it.kind));
        next = { ...got, id: it.id, ...(same ? { kind: it.kind, label: it.label } : {}) };
      } catch (e) {
        return setChange({ ...change, busy: false, error: (e as Error).message });
      }
    }
    const list = latest.current.items.map((x, k) => (k === i ? next! : x));
    // The part on screen: its new version goes on screen now (asked first while live).
    const r = await saveService({ ...latest.current, items: list }, i === cur);
    if (r.ok) {
      setChange(null);
      toast.success(`${itemLabel(next)} gewijzigd`);
    } else setChange({ ...change, busy: false });
  }

  function tap(i: number) {
    if (drag) return;
    setSel(sel === i ? null : i);
    const it = items[i];
    setChange(CHANGEABLE.has(it.kind) && change?.i !== i ? { i, text: prefill(it) } : null);
  }

  // Dragging by the handle, with mouse or finger: the row follows, the others make room.
  function down(i: number, e: React.PointerEvent) {
    if (LOCKED.has(items[i].kind)) return;
    e.preventDefault();
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    const tops = items.map((_, k) => rows.current[k]?.offsetTop ?? 0);
    const hs = items.map((_, k) => rows.current[k]?.offsetHeight ?? 0);
    setChange(null);
    setDrag({ from: i, y0: e.clientY, s0: box.current?.scrollTop ?? 0, tops, hs, dy: 0, to: i });
  }
  function moveDrag(e: React.PointerEvent) {
    if (!drag) return;
    const b = box.current;
    if (b) {
      const r = b.getBoundingClientRect();
      if (e.clientY < r.top + 60) b.scrollTop -= 14;
      else if (e.clientY > r.bottom - 60) b.scrollTop += 14;
    }
    const dy = e.clientY - drag.y0 + ((b?.scrollTop ?? 0) - drag.s0);
    const mid = drag.tops[drag.from] + drag.hs[drag.from] / 2 + dy;
    let to = 0;
    drag.tops.forEach((t, k) => {
      if (k !== drag.from && t + drag.hs[k] / 2 < mid) to++;
    });
    to = Math.max(lead, Math.min(to, items.length - 1 - tail));
    setDrag({ ...drag, dy, to });
  }
  function up() {
    if (!drag) return;
    const { from, to } = drag;
    setDrag(null);
    if (to !== from) moveTo(from, to);
  }
  function shift(k: number) {
    if (!drag || k === drag.from) return 0;
    const h = drag.hs[drag.from];
    if (drag.from < drag.to && k > drag.from && k <= drag.to) return -h;
    if (drag.from > drag.to && k < drag.from && k >= drag.to) return h;
    return 0;
  }

  const ctl = "grid size-14 shrink-0 place-items-center rounded-xl text-walnut transition hover:bg-accent disabled:text-[#c4b59a] disabled:hover:bg-transparent [&_svg]:size-7";

  return (
    <div className="flex min-h-0 flex-1 flex-col rounded-2xl border-[3px] border-dashed border-primary bg-[#fbf6ea]">
      <div className="flex shrink-0 items-center gap-3 border-b-2 border-primary/30 bg-[#f7f0de] px-3 py-2">
        <label className="flex h-16 min-w-0 flex-1 items-center gap-3 rounded-xl border-2 border-input bg-card pl-4 focus-within:border-primary">
          <span className="shrink-0 text-[20px] whitespace-nowrap text-muted-foreground">na {itemLabel(items[spot - 1]) || "het begin"}</span>
          <input
            value={typed}
            onChange={(e) => setTyped(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && add()}
            placeholder="ps 84:3, gebed"
            aria-label="Toevoegen"
            className="h-full min-w-0 flex-1 bg-transparent pr-3 text-[24px] outline-none placeholder:text-[#a8987f]"
          />
        </label>
        <Button onClick={add} disabled={busy?.spin || !typed.trim()} className="h-16 shrink-0">
          {busy?.spin ? <LoaderCircle className="animate-spin" /> : <Plus />} Voeg toe
        </Button>
      </div>
      {busy && !busy.spin && (
        <div className={cn("shrink-0 truncate px-4 pt-1 text-[20px] font-semibold", busy.bad ? "text-live" : "text-ok")}>{busy.text}</div>
      )}
      <div ref={box} className="scroll-thin min-h-0 flex-1 overflow-y-auto p-2">
        <ol className="relative">
          {items.map((it, i) => {
            const locked = LOCKED.has(it.kind);
            const dragging = drag?.from === i;
            const ch = change?.i === i ? change : null;
            return (
              <li
                key={it.id || i}
                ref={(el) => { rows.current[i] = el; }}
                style={{ transform: `translateY(${dragging ? drag!.dy : shift(i)}px)` }}
                className={cn(
                  "relative rounded-xl",
                  dragging ? "z-10 bg-[#fdf8ec] shadow-[0_12px_30px_-10px_rgb(90_62_43/0.6)] ring-2 ring-primary" : drag && "transition-transform duration-150",
                  sel === i && !dragging && "bg-primary/15",
                )}
              >
                <div className="flex min-h-[64px] items-center gap-1">
                  {locked ? (
                    <span className="grid size-14 shrink-0 place-items-center text-muted-foreground"><Lock className="size-6" /></span>
                  ) : (
                    <span
                      role="button"
                      aria-label="Slepen"
                      onPointerDown={(e) => down(i, e)}
                      onPointerMove={moveDrag}
                      onPointerUp={up}
                      onPointerCancel={up}
                      className="grid size-14 shrink-0 cursor-grab touch-none place-items-center rounded-xl text-walnut hover:bg-accent active:cursor-grabbing"
                    >
                      <GripVertical className="size-8" />
                    </span>
                  )}
                  <button
                    type="button"
                    disabled={locked}
                    onClick={() => tap(i)}
                    className={cn("flex min-w-0 flex-1 items-baseline gap-4 rounded-xl px-2 py-2 text-left disabled:text-muted-foreground", i === cur && "font-semibold")}
                  >
                    {i === cur && <span className="shrink-0 text-primary">▶</span>}
                    <span className="shrink-0 text-[26px]">{itemLabel(it)}</span>
                    {itemDetail(it) && <span className="min-w-0 truncate text-[24px] text-[#4a3826]">{itemDetail(it)}</span>}
                  </button>
                  {!locked && (
                    <>
                      <button type="button" aria-label="Omhoog" className={ctl} disabled={i <= lead} onClick={() => moveTo(i, i - 1)}><ArrowUp /></button>
                      <button type="button" aria-label="Omlaag" className={ctl} disabled={i >= items.length - 1 - tail} onClick={() => moveTo(i, i + 1)}><ArrowDown /></button>
                      <button type="button" aria-label="Weghalen" className={cn(ctl, "text-live")} disabled={i === cur} onClick={() => remove(i)}><X strokeWidth={3} /></button>
                    </>
                  )}
                </div>
                {ch && (
                  <div className="flex flex-wrap items-center gap-3 px-3 pb-3 pl-16">
                    <Input
                      autoFocus
                      value={ch.text}
                      onChange={(e) => setChange({ ...ch, text: e.target.value, error: "" })}
                      onKeyDown={(e) => e.key === "Enter" && replace()}
                      onFocus={(e) => e.currentTarget.select()}
                      aria-label="Wijzigen in"
                      className="h-16 w-[280px] min-w-0 flex-1 text-[26px]"
                    />
                    <Button className="h-16 shrink-0" disabled={ch.busy || !ch.text.trim()} onClick={replace}>
                      {ch.busy && <LoaderCircle className="animate-spin" />} {i === cur ? "Opslaan" : "Vervangen"}
                    </Button>
                    <Button variant="ghost" className="h-16 shrink-0" onClick={() => setChange(null)}>Annuleer</Button>
                    {ch.error && <span className="w-full text-[20px] font-semibold text-live">{ch.error}</span>}
                  </div>
                )}
              </li>
            );
          })}
        </ol>
      </div>
    </div>
  );
}
