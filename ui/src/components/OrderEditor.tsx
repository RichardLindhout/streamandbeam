import { useRef, useState } from "react";
import { ArrowDown, ArrowUp, CircleAlert, LoaderCircle, Lock, Pencil, Plus, RotateCcw, Trash, X } from "lucide-react";
import { toast } from "sonner";
import type { CustomSlide, Item, Service, View } from "@/lib/api";
import { quick, saveService } from "@/lib/api";
import { KindIcon, itemDetail, itemLabel } from "@/lib/kinds";
import { cn } from "@/lib/utils";
import { Popover, PopoverContent, PopoverTrigger } from "./ui/popover";
import { Card, CardHeader } from "./ui/card";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Textarea } from "./ui/textarea";
import { BlurField } from "./BlurField";

const LOCKED = new Set(["welkom", "einde"]);

function MenuItem({ icon, onClick, children, danger, disabled }: { icon: React.ReactNode; onClick: () => void; children: React.ReactNode; danger?: boolean; disabled?: boolean }) {
  return (
    <button
      type="button"
      disabled={disabled}
      onClick={onClick}
      className={cn(
        "flex min-h-[72px] w-full items-center gap-4 rounded-xl px-4 text-left text-[26px] transition hover:bg-accent disabled:text-[#a8987f] [&_svg]:size-7 [&_svg]:text-walnut disabled:[&_svg]:text-[#c4b59a]",
        danger && "text-live [&_svg]:text-live",
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
  const [slides, setSlides] = useState(initial.length ? initial : [{ header: label, sub: "", text: "", footer: "" }]);
  const set = (k: number, patch: Partial<CustomSlide>) => setSlides(slides.map((c, j) => (j === k ? { ...c, ...patch } : c)));
  return (
    <div className="mx-2 mb-3 grid gap-4 rounded-2xl border-2 border-primary bg-[#fdf8ec] p-5">
      {slides.map((c, k) => (
        <div key={k} className="grid gap-3 rounded-xl border-2 bg-card p-4">
          <div className="flex items-center gap-3">
            <span className="text-[18px] font-bold whitespace-nowrap text-muted-foreground">Dia {k + 1}</span>
            <Input value={c.header} placeholder="Kop" onChange={(e) => set(k, { header: e.target.value })} />
            {slides.length > 1 && (
              <Button variant="ghost" size="icon-sm" aria-label="Dia weghalen" onClick={() => setSlides(slides.filter((_, j) => j !== k))}><X /></Button>
            )}
          </div>
          <Textarea rows={6} value={c.text} onChange={(e) => set(k, { text: e.target.value })} />
        </div>
      ))}
      <div className="flex flex-wrap gap-3">
        <Button onClick={() => onSave(slides)}>Opslaan</Button>
        <Button variant="outline" onClick={() => setSlides([...slides, { header: label, sub: "", text: "", footer: "" }])}><Plus /> Dia</Button>
        <Button variant="outline" onClick={onReset}><RotateCcw /> Origineel</Button>
        <Button variant="ghost" onClick={onClose}>Annuleer</Button>
      </div>
    </div>
  );
}

/** Where typed parts go by default: before the blessing and the end card. */
function defaultSpot(items: Item[]) {
  let at = items.findIndex((i) => i.kind === "einde");
  if (at < 0) at = items.length;
  if (at > 0 && items[at - 1]?.kind === "zegen") at--;
  return at;
}

export function OrderEditor({ svc, state }: { svc: Service; state: View | null }) {
  const [typed, setTyped] = useState("");
  const [busy, setBusy] = useState<{ text: string; spin?: boolean; bad?: boolean } | null>(null);
  const [after, setAfter] = useState<number | null>(null);
  const [edit, setEdit] = useState<{ i: number; slides: CustomSlide[] } | null>(null);
  const [menu, setMenu] = useState<number | null>(null);
  const latest = useRef(svc);
  latest.current = svc;
  const isCurrent = state?.current === svc.id;
  const cur = isCurrent ? state?.slides?.[state.index]?.item : undefined;

  const update = (items: Item[]) => saveService({ ...latest.current, items });
  const spot = after !== null && svc.items[after] ? after + 1 : defaultSpot(svc.items);
  const comesAfter = itemLabel(svc.items[spot - 1]);

  async function add() {
    if (!typed.trim()) return;
    setBusy({ text: "Bezig…", spin: true });
    let items: Item[];
    try {
      items = (await quick(typed)).map((it, k) => ({ ...it, id: "i" + Date.now() + k }));
    } catch (e) {
      return setBusy({ text: (e as Error).message, bad: true });
    }
    const list = [...latest.current.items];
    list.splice(spot, 0, ...items);
    setAfter(spot + items.length - 1);
    update(list);
    const unsure = items.filter((i) => i.problem).length;
    setBusy({ text: `${items.length} toegevoegd` + (unsure ? ` · ${unsure} nakijken` : ""), bad: unsure > 0 });
    setTyped("");
  }
  function move(i: number, d: number) {
    setMenu(null);
    const j = i + d;
    if (j < 0 || j >= svc.items.length || LOCKED.has(svc.items[j].kind)) return;
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
    toast(`${itemLabel(gone)} weggehaald`, {
      duration: 8000,
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
  function openEdit(i: number) {
    setMenu(null);
    const it = svc.items[i];
    const slides = it.custom?.length
      ? it.custom.map((c) => ({ ...c }))
      : (isCurrent ? state?.slides || [] : []).filter((s) => s.item === i).map((s) => ({
          header: s.header, sub: s.sub, footer: s.footer || "",
          text: [...(s.lines || []), ...(s.verses || []).map((v) => v.verse + " " + v.text), ...(s.theme ? [s.theme] : []), ...(s.points || []).map((p, k) => `${k + 1}. ${p}`), ...(s.list || [])].join("\n"),
        }));
    setEdit({ i, slides });
  }
  const setItem = (i: number, patch: Partial<Item>) => update(latest.current.items.map((x, k) => (k === i ? { ...x, ...patch } : x)));

  return (
    <Card className="overflow-hidden">
      <CardHeader>Volgorde</CardHeader>

      <div className="grid gap-3 border-b-2 bg-[#f7f0de] p-5">
        <div className="flex gap-3">
          <Input
            value={typed}
            onChange={(e) => setTyped(e.target.value)}
            onKeyDown={(e) => e.key === "Enter" && add()}
            placeholder="Typ wat er komt"
            className="h-[72px] text-[24px]"
          />
          <Button onClick={add} disabled={busy?.spin || !typed.trim()} className="shrink-0">
            {busy?.spin ? <LoaderCircle className="animate-spin" /> : <Plus />} Voeg toe
          </Button>
        </div>
        <div className="flex flex-wrap items-center gap-x-4 gap-y-1 text-[22px]">
          <span>Komt na: <b>{comesAfter || "het begin"}</b></span>
          {busy && <span className={cn("ml-auto font-semibold", busy.bad ? "text-live" : "text-ok")}>{busy.text}</span>}
        </div>
        {(svc.problems || []).length > 0 && (
          <div className="flex gap-3 rounded-xl border-2 border-warn/50 bg-warn/10 p-3 text-[20px] text-[#7a4a00]">
            <CircleAlert className="mt-0.5 size-6 shrink-0" /> Niet herkend: {svc.problems.join("; ")}
          </div>
        )}
      </div>

      <ol className="p-2">
        {svc.items.map((it, i) => {
          const locked = LOCKED.has(it.kind);
          const row = (
            <span className={cn("flex min-h-[72px] w-full items-center gap-4 rounded-xl px-4 py-2 text-left", !locked && (menu === i ? "bg-accent" : "hover:bg-accent"))}>
              <span className={cn("grid size-11 shrink-0 place-items-center rounded-lg", i === cur ? "bg-primary text-primary-foreground" : "bg-secondary text-walnut")}>
                <KindIcon kind={it.kind} className="size-6" />
              </span>
              <span className="min-w-0 flex-1 leading-tight">
                <span className="flex items-baseline gap-3">
                  <span className="truncate text-[26px] font-semibold">{itemLabel(it)}</span>
                  {itemDetail(it) && <span className="truncate text-[24px] text-muted-foreground">{itemDetail(it)}</span>}
                  {it.custom?.length ? <span className="hymnal shrink-0 text-[18px] text-[#7a5a1c]">aangepast</span> : null}
                </span>
                {it.problem && <span className="mt-1 flex items-center gap-2 text-[20px] text-[#9a5a00]"><CircleAlert className="size-5" /> {it.problem}</span>}
              </span>
              {locked && <Lock className="size-5 shrink-0 text-muted-foreground" />}
            </span>
          );
          return (
            <li key={it.id || i} className={cn("rounded-xl", after === i && "bg-primary/10")}>
              {locked ? (
                row
              ) : (
                <Popover open={menu === i} onOpenChange={(o) => setMenu(o ? i : null)}>
                  <PopoverTrigger asChild>
                    <button type="button" className="block w-full">{row}</button>
                  </PopoverTrigger>
                  <PopoverContent align="end" side="bottom" className="w-80 p-2">
                    <MenuItem icon={<Pencil />} onClick={() => openEdit(i)}>Tekst aanpassen</MenuItem>
                    <MenuItem icon={<ArrowUp />} disabled={i === 0 || LOCKED.has(svc.items[i - 1]?.kind)} onClick={() => move(i, -1)}>Omhoog</MenuItem>
                    <MenuItem icon={<ArrowDown />} disabled={i === svc.items.length - 1 || LOCKED.has(svc.items[i + 1]?.kind)} onClick={() => move(i, 1)}>Omlaag</MenuItem>
                    <div className="my-1 h-0.5 bg-border" />
                    <MenuItem icon={<Trash />} danger onClick={() => remove(i)}>Weghalen</MenuItem>
                  </PopoverContent>
                </Popover>
              )}
              {(it.kind === "collecte" || it.kind === "mededeling") && (
                <div className="px-4 pb-3 pl-[4.75rem]">
                  <BlurField
                    multiline
                    rows={Math.min(6, Math.max(2, (it.text || "").split("\n").length))}
                    value={it.text}
                    placeholder={it.kind === "collecte" ? "Collectedoelen" : "Tekst"}
                    onSave={(v) => setItem(i, { text: v })}
                  />
                </div>
              )}
              {edit?.i === i && (
                <SlideEditor
                  key={it.id}
                  initial={edit.slides}
                  label={itemLabel(it)}
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
