import { useRef, useState } from "react";
import { ArrowDown, ArrowUp, CircleAlert, CornerDownRight, ListOrdered, LoaderCircle, Pencil, Play, Plus, RotateCcw, Sparkles, Trash, X } from "lucide-react";
import { Popover, PopoverContent, PopoverTrigger } from "./ui/popover";
import { toast } from "sonner";
import type { CustomSlide, Item, Service, View } from "@/lib/api";
import { go, quick, saveService } from "@/lib/api";
import { KindIcon } from "@/lib/kinds";
import { cn } from "@/lib/utils";
import { Card, CardHeader } from "./ui/card";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Textarea } from "./ui/textarea";
import { BlurField } from "./BlurField";

function MenuItem({ icon, onClick, children, danger, disabled }: { icon: React.ReactNode; onClick: () => void; children: React.ReactNode; danger?: boolean; disabled?: boolean }) {
  return (
    <button
      type="button"
      disabled={disabled}
      onClick={onClick}
      className={cn(
        "flex w-full items-center gap-3 rounded-lg px-3 py-2.5 text-left text-[15px] transition hover:bg-accent disabled:opacity-30 [&_svg]:size-4 [&_svg]:text-primary",
        danger && "text-[#f3a3a8] [&_svg]:text-live",
      )}
    >
      {icon}
      {children}
    </button>
  );
}

/** Edits the texts of one part of the service as its own slides. */
function SlideEditor({ initial, label, onSave, onReset, onClose }: {
  initial: CustomSlide[]; label: string; onSave: (s: CustomSlide[]) => void; onReset: () => void; onClose: () => void;
}) {
  const [slides, setSlides] = useState(initial);
  const set = (k: number, patch: Partial<CustomSlide>) => setSlides(slides.map((c, j) => (j === k ? { ...c, ...patch } : c)));
  return (
    <div className="mx-2 mb-3 grid gap-3 rounded-xl border border-primary/40 bg-background/70 p-4" onClick={(e) => e.stopPropagation()}>
      <div className="text-xs font-semibold text-primary">Tekst aanpassen — {label}</div>
      {slides.map((c, k) => (
        <div key={k} className="grid gap-2 rounded-lg border bg-card p-3">
          <div className="flex items-center gap-2">
            <span className="text-[11px] font-bold whitespace-nowrap text-muted-foreground">DIA {k + 1}</span>
            <Input className="h-9" value={c.header} placeholder="Kop" onChange={(e) => set(k, { header: e.target.value })} />
            {slides.length > 1 && (
              <Button variant="ghost" size="icon" title="Dia weghalen" onClick={() => setSlides(slides.filter((_, j) => j !== k))}><X /></Button>
            )}
          </div>
          <Textarea rows={6} value={c.text} onChange={(e) => set(k, { text: e.target.value })} />
        </div>
      ))}
      <div className="flex flex-wrap gap-2">
        <Button onClick={() => onSave(slides)}>Opslaan</Button>
        <Button variant="outline" onClick={() => setSlides([...slides, { header: label, sub: "", text: "", footer: "" }])}><Plus /> Dia</Button>
        <Button variant="outline" onClick={onReset}><RotateCcw /> Herstel origineel</Button>
        <Button variant="ghost" onClick={onClose}>Annuleer</Button>
      </div>
    </div>
  );
}

export function OrderEditor({ svc, state }: { svc: Service; state: View }) {
  const [typed, setTyped] = useState("");
  const [busy, setBusy] = useState<{ text: string; spin?: boolean } | null>(null);
  const [after, setAfter] = useState<number | null>(null);
  const [edit, setEdit] = useState<{ i: number; slides: CustomSlide[] } | null>(null);
  const [menu, setMenu] = useState<number | null>(null);
  const latest = useRef(svc);
  latest.current = svc;
  const cur = state.slides?.[state.index]?.item;

  const update = (items: Item[]) => saveService({ ...latest.current, items });

  async function add() {
    if (!typed.trim()) return;
    setBusy({ text: "Bezig met herkennen…", spin: true });
    let items: Item[];
    try {
      items = (await quick(typed)).map((it, k) => ({ ...it, id: "i" + Date.now() + k }));
    } catch (e) {
      return setBusy({ text: "Fout: " + (e as Error).message });
    }
    const list = [...latest.current.items];
    let at = after !== null ? after + 1 : list.findIndex((i) => i.kind === "einde");
    if (at < 0) at = list.length;
    list.splice(at, 0, ...items);
    setAfter(at + items.length - 1);
    update(list);
    const unsure = items.filter((i) => i.problem).length;
    setBusy({ text: `${items.length} toegevoegd` + (unsure ? ` — ${unsure} niet zeker, zie ⚠` : "") });
    setTyped("");
  }
  function move(i: number, d: number) {
    setMenu(null);
    const j = i + d;
    if (j < 0 || j >= svc.items.length) return;
    const list = [...svc.items];
    [list[i], list[j]] = [list[j], list[i]];
    update(list);
    if (after === i) setAfter(j);
  }
  function remove(i: number) {
    setMenu(null);
    const list = [...svc.items];
    const [gone] = list.splice(i, 1);
    update(list);
    setAfter(null);
    toast(`“${gone.label}” weggehaald`, {
      duration: 8000,
      action: {
        label: "Ongedaan maken",
        onClick: () => {
          const l = [...latest.current.items];
          l.splice(i, 0, gone);
          update(l);
        },
      },
    });
  }
  function openEdit(i: number) {
    setMenu(null);
    const it = svc.items[i];
    const slides = it.custom?.length
      ? it.custom.map((c) => ({ ...c }))
      : (state.slides || []).filter((s) => s.item === i).map((s) => ({
          header: s.header, sub: s.sub, footer: s.footer || "",
          text: [...(s.lines || []), ...(s.verses || []).map((v) => v.verse + " " + v.text), ...(s.theme ? [s.theme] : []), ...(s.points || []).map((p, k) => `${k + 1}. ${p}`), ...(s.list || [])].join("\n"),
        }));
    setEdit({ i, slides });
  }
  const setItem = (i: number, patch: Partial<Item>) => update(latest.current.items.map((x, k) => (k === i ? { ...x, ...patch } : x)));

  return (
    <Card className="overflow-hidden">
      <CardHeader action={<span className="text-[13px] text-muted-foreground">tik op een onderdeel om het te wijzigen</span>}>
        <ListOrdered /> Volgorde van de dienst
      </CardHeader>

      <div className="grid gap-2 border-b bg-background/30 p-4">
        <div className="flex gap-2">
          <Input
            className="h-12 text-base"
            value={typed}
            onChange={(e) => setTyped(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && add()}
            placeholder="Typ wat er komt: ps 42:5, gebed, joh 3:16, HC 57, NGB art. 14"
          />
          <Button size="lg" className="h-12" onClick={add} disabled={busy?.spin}>
            {busy?.spin ? <LoaderCircle className="animate-spin" /> : <Sparkles />} Voeg toe
          </Button>
        </div>
        <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-[13px] text-muted-foreground">
          <span>
            Komt {after !== null && svc.items[after] ? <>na <b className="text-foreground">{svc.items[after].label}</b></> : <>vóór het einde</>}
            {after !== null && <button className="ml-2 text-primary hover:underline" onClick={() => setAfter(null)}>vóór het einde</button>}
          </span>
          {busy && <span className={cn("ml-auto font-medium", busy.text.startsWith("Fout") ? "text-live" : "text-foreground")}>{busy.text}</span>}
        </div>
        {(svc.problems || []).length > 0 && (
          <div className="flex gap-2 rounded-lg border border-warn/40 bg-warn/10 p-3 text-[13px] text-warn">
            <CircleAlert className="size-4 shrink-0" /> Niet herkend op de website: {svc.problems.join("; ")}
          </div>
        )}
      </div>

      <ol className="p-2">
        {svc.items.map((it, i) => {
          const sel = after === i;
          return (
            <li key={it.id || i} className={cn("rounded-xl transition-colors", sel && "bg-primary/10 ring-1 ring-primary/40 ring-inset")}>
              <Popover open={menu === i} onOpenChange={(o) => setMenu(o ? i : null)}>
                <PopoverTrigger asChild>
                  <button className={cn("flex w-full items-center gap-3 rounded-xl px-3 py-2.5 text-left", menu === i ? "bg-accent" : "hover:bg-accent")}>
                    <span className="w-6 text-right text-sm text-muted-foreground tabular-nums">{i + 1}</span>
                    <span className={cn("grid size-8 shrink-0 place-items-center rounded-lg", i === cur ? "bg-primary text-primary-foreground" : "bg-secondary text-muted-foreground")}>
                      <KindIcon kind={it.kind} className="size-4" />
                    </span>
                    <span className="min-w-0 flex-1 leading-tight">
                      <span className="flex items-center gap-2">
                        <span className="truncate text-[16px] font-semibold">{it.label}</span>
                        {it.ref && <span className="truncate text-[15px] text-muted-foreground">{it.ref}</span>}
                        {it.custom?.length ? <span className="hymnal text-[12px] text-primary">aangepast</span> : null}
                      </span>
                      {it.problem && <span className="mt-0.5 flex items-center gap-1 text-[13px] text-warn"><CircleAlert className="size-3.5" /> {it.problem}</span>}
                    </span>
                    {sel && <span className="hymnal text-[12px] text-primary">hierna invoegen</span>}
                  </button>
                </PopoverTrigger>
                <PopoverContent align="end" side="bottom" className="w-64 p-1.5">
                  <div className="truncate px-3 pt-2 pb-1.5 font-serif text-[16px] font-semibold">{it.label}</div>
                  <MenuItem icon={<Play />} onClick={() => { setMenu(null); const s = state.slides.findIndex((x) => x.item === i); if (s >= 0) go({ index: s }); }}>Nu tonen</MenuItem>
                  <MenuItem icon={<Pencil />} onClick={() => openEdit(i)}>Tekst aanpassen</MenuItem>
                  <MenuItem icon={<ArrowUp />} disabled={i === 0} onClick={() => move(i, -1)}>Omhoog</MenuItem>
                  <MenuItem icon={<ArrowDown />} disabled={i === svc.items.length - 1} onClick={() => move(i, 1)}>Omlaag</MenuItem>
                  <MenuItem icon={<CornerDownRight />} onClick={() => { setAfter(i); setMenu(null); }}>Nieuwe onderdelen hierna</MenuItem>
                  <div className="my-1 h-px bg-border" />
                  <MenuItem icon={<Trash />} danger onClick={() => remove(i)}>Weghalen</MenuItem>
                </PopoverContent>
              </Popover>
              {(it.kind === "collecte" || it.kind === "mededeling") && (
                <div className="px-3 pb-3 pl-[4.75rem]">
                  <BlurField
                    multiline
                    rows={Math.min(6, Math.max(2, (it.text || "").split("\n").length))}
                    value={it.text}
                    placeholder={it.kind === "collecte" ? "Collectedoelen, één per regel" : "Tekst (mag leeg)"}
                    onSave={(v) => setItem(i, { text: v })}
                  />
                </div>
              )}
              {edit?.i === i && (
                <SlideEditor
                  key={it.id}
                  initial={edit.slides}
                  label={it.label}
                  onSave={(s) => { setItem(i, { custom: s }); setEdit(null); }}
                  onReset={() => { setItem(i, { custom: [] }); setEdit(null); }}
                  onClose={() => setEdit(null)}
                />
              )}
            </li>
          );
        })}
      </ol>
    </Card>
  );
}
