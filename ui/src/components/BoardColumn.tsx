import { useLayoutEffect, useRef, useState } from "react";
import { X } from "lucide-react";
import type { BoardCustom, Service, Slide, View } from "@/lib/api";
import { go, post, saveService } from "@/lib/api";
import { itemLabel } from "@/lib/kinds";
import { cn } from "@/lib/utils";
import { Board } from "./Board";
import { Confirm } from "./Confirm";
import { Button } from "./ui/button";
import { Input } from "./ui/input";
import { Label } from "./ui/label";
import { Textarea } from "./ui/textarea";
import { Dialog, DialogBody, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "./ui/dialog";

type Kind = "bord" | "thema" | "slotzang";
const KINDS: { kind: Kind; label: string; key: keyof BoardCustom }[] = [
  { kind: "bord", label: "Liturgie", key: "liturgie" },
  { kind: "thema", label: "Thema", key: "thema" },
  { kind: "slotzang", label: "Slotzang", key: "slotzang" },
];

/** The board slide of this kind nearest after the current one (else the first), with its index. */
function pick(boards: Slide[], kind: string, index: number): [number, Slide] | null {
  if (boards[index]?.kind === kind) return [index, boards[index]];
  for (let i = index + 1; i < boards.length; i++) if (boards[i].kind === kind) return [i, boards[i]];
  const i = boards.findIndex((b) => b.kind === kind);
  return i >= 0 ? [i, boards[i]] : null;
}

const NAME = 30; // the name under a tile
const PAD = 12;
const GAP = 14;

function Tile({ w, label, on, missing, custom, edit, onClick, onRemove, children }: {
  w: number; label: string; on: boolean; missing?: boolean; custom?: boolean; edit?: boolean;
  onClick?: () => void; onRemove?: () => void; children: React.ReactNode;
}) {
  return (
    <div className="relative flex flex-col items-center" style={{ width: w }}>
      <button
        type="button"
        disabled={missing || !onClick || (on && !edit)}
        onClick={onClick}
        className={cn(
          "relative block rounded-lg transition disabled:cursor-default",
          on ? "ring-[5px] ring-primary shadow-[0_0_30px_-6px_var(--primary)]" : "ring-1 ring-walnut/30 hover:ring-[3px] hover:ring-primary/60",
          edit && !missing && "outline-2 outline-offset-2 outline-primary outline-dashed",
        )}
        style={{ width: w, height: (w * 16) / 9 }}
      >
        <div className={cn("h-full w-full overflow-hidden rounded-lg", !on && !edit && "opacity-60", missing && "!opacity-20")}>{children}</div>
        {on && (
          <span className="hymnal absolute -top-3 left-1/2 -translate-x-1/2 rounded-md bg-primary px-2 text-[18px] leading-[26px] whitespace-nowrap text-primary-foreground shadow-md">
            op het bord
          </span>
        )}
        {missing && <span className="absolute inset-0 grid place-items-center px-1 text-center text-[18px] leading-tight font-semibold text-muted-foreground">niet in deze dienst</span>}
        {!missing && (edit || custom) && (
          <span className="absolute inset-x-1 bottom-1 grid gap-1">
            {custom && <span className="hymnal mx-auto rounded bg-[#fdf8ec] px-1.5 text-[18px] leading-[24px] text-[#7a5a1c]">aangepast</span>}
            {edit && <span className="rounded-md bg-primary py-1 text-[18px] font-semibold text-primary-foreground">Aanpassen</span>}
          </span>
        )}
      </button>
      {onRemove && (
        <button
          type="button"
          aria-label="Eigen tekst weghalen"
          onClick={onRemove}
          className="absolute -top-3 -right-3 z-10 grid size-11 place-items-center rounded-full bg-walnut text-white shadow-lg hover:bg-[#6b4a33]"
        >
          <X className="size-6" strokeWidth={3} />
        </button>
      )}
      <div className={cn("truncate text-center text-[18px] leading-[30px]", on ? "font-semibold text-[#7a5a1c]" : "text-muted-foreground")} style={{ width: w + 16 }}>
        {label}
      </div>
    </div>
  );
}

/** One board's own text: the psalm board's rows, the theme with its points, or the slotzang. */
function BoardEditor({ kind, svc, slide, onScreen, onClose }: { kind: Kind | null; svc?: Service; slide?: Slide; onScreen: boolean; onClose: () => void }) {
  const custom = svc?.boardCustom || {};
  const [rows, setRows] = useState("");
  const [theme, setTheme] = useState({ tekst: "", thema: "", punten: "" });
  const [song, setSong] = useState("");
  const [opened, setOpened] = useState<Kind | null>(null);
  if (kind !== opened) {
    // Fill the fields with what the board shows now, once each time it opens.
    setOpened(kind);
    setRows((custom.liturgie ?? slide?.list ?? []).join("\n"));
    const t = custom.thema;
    setTheme({ tekst: t?.tekst ?? slide?.sub ?? "", thema: t?.thema ?? slide?.theme ?? "", punten: (t?.punten ?? slide?.points ?? []).join("\n") });
    setSong(custom.slotzang ?? slide?.sub ?? "");
  }
  if (!svc) return null;
  const meta = KINDS.find((k) => k.kind === kind);
  const own = meta ? custom[meta.key] != null : false;
  const save = async (patch: Partial<BoardCustom>) => {
    // The board on the TVs now changes on Opslaan (asked first while live); the others wait.
    const r = await saveService({ ...svc, boardCustom: { ...custom, ...patch } }, onScreen);
    if (r.ok) onClose();
  };
  const lines = (s: string) => {
    const l = s.split("\n").map((x) => x.trimEnd());
    while (l.length && !l[l.length - 1]) l.pop();
    return l;
  };
  function store() {
    if (kind === "bord") save({ liturgie: lines(rows) });
    else if (kind === "thema") save({ thema: { tekst: theme.tekst.trim(), thema: theme.thema.trim(), punten: lines(theme.punten).filter(Boolean) } });
    else if (kind === "slotzang") save({ slotzang: song.trim() });
  }
  return (
    <Dialog open={!!kind} onOpenChange={(o) => !o && onClose()}>
      <DialogContent className="max-w-2xl">
        <DialogHeader>
          <DialogTitle>{meta?.label}</DialogTitle>
          <DialogDescription className="sr-only">Bord aanpassen</DialogDescription>
        </DialogHeader>
        <DialogBody>
          {kind === "bord" && (
            <div className="grid gap-2">
              <Label htmlFor="bord-rijen">Eén regel per plank</Label>
              <Textarea id="bord-rijen" rows={12} value={rows} onChange={(e) => setRows(e.target.value)} className="font-mono text-[24px]" />
            </div>
          )}
          {kind === "thema" && (
            <>
              <div className="grid gap-2"><Label htmlFor="bt-tekst">Tekst</Label><Input id="bt-tekst" value={theme.tekst} onChange={(e) => setTheme({ ...theme, tekst: e.target.value })} /></div>
              <div className="grid gap-2"><Label htmlFor="bt-thema">Thema</Label><Input id="bt-thema" value={theme.thema} onChange={(e) => setTheme({ ...theme, thema: e.target.value })} /></div>
              <div className="grid gap-2"><Label htmlFor="bt-punten">Punten</Label><Textarea id="bt-punten" rows={4} value={theme.punten} onChange={(e) => setTheme({ ...theme, punten: e.target.value })} /></div>
            </>
          )}
          {kind === "slotzang" && (
            <div className="grid gap-2"><Label htmlFor="bs-tekst">Slotzang</Label><Input id="bs-tekst" value={song} onChange={(e) => setSong(e.target.value)} className="text-[26px]" /></div>
          )}
        </DialogBody>
        <DialogFooter>
          {own && meta && (
            <Button variant="outline" size="lg" className="mr-auto" onClick={() => save({ [meta.key]: null })}>
              Terug naar automatisch
            </Button>
          )}
          <Button size="lg" onClick={store}>Opslaan</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

/** The church boards beside the order: the one on the TVs has a gold edge. Tap another to put it up;
 *  in edit mode tap one to give it your own text. */
export function BoardColumn({ state, svc, edit }: { state: View | null; svc?: Service; edit: boolean }) {
  const box = useRef<HTMLDivElement>(null);
  const [h, setH] = useState(500);
  const [ask, setAsk] = useState<{ index: number; label: string; to: string } | null>(null);
  const [editing, setEditing] = useState<Kind | null>(null);
  useLayoutEffect(() => {
    const el = box.current;
    if (!el) return;
    const ro = new ResizeObserver(() => setH(el.clientHeight));
    ro.observe(el);
    setH(el.clientHeight);
    return () => ro.disconnect();
  }, []);
  // Two rows of two upright boards fill the height; the width follows.
  const w = Math.round(Math.max(70, Math.min(240, ((h - 2 * PAD - GAP - 2 * NAME - 16) / 2) * (9 / 16))));

  const boards = state?.boards?.length ? state.boards : state?.slides || [];
  const index = state?.index ?? 0;
  const override = state?.overrideBoard;
  const current = override ? "vrij" : state?.black ? "zwart" : boards[index]?.kind;
  const custom = svc?.boardCustom || {};
  const picks = Object.fromEntries(KINDS.map(({ kind }) => [kind, pick(boards, kind, index)]));

  return (
    <aside
      ref={box}
      className="flex h-full shrink-0 flex-col items-center border-l-2 border-walnut/15 bg-[#efe5cc]/60"
      style={{ width: 2 * w + 3 * PAD + 8, padding: PAD, paddingTop: PAD + 6 }}
    >
      <div className="grid grid-cols-2 content-start" style={{ columnGap: PAD, rowGap: GAP }}>
        {KINDS.map(({ kind, label, key }) => {
          const p = picks[kind];
          const target = p ? itemLabel(svc?.items?.[p[1].item]) : "";
          return (
            <Tile
              key={kind}
              w={w}
              label={label}
              on={!edit && current === kind}
              missing={!p}
              custom={custom[key] != null}
              edit={edit}
              onClick={p ? () => (edit ? setEditing(kind) : setAsk({ index: p[0], label, to: target })) : undefined}
            >
              <Board slide={p?.[1]} className="h-full w-full" />
            </Tile>
          );
        })}
        {override && (
          <Tile w={w} label="Eigen tekst" on={!edit} onRemove={() => post("/api/board-override", { clear: true })}>
            <Board slide={override} className="h-full w-full" />
          </Tile>
        )}
      </div>
      <Confirm
        open={!!ask}
        onOpenChange={(o) => !o && setAsk(null)}
        title={ask ? `${ask.label} op het bord?` : ""}
        text={ask?.to ? `Gaat naar ${ask.to}` : undefined}
        yes="Ja"
        tone="gold"
        onYes={() => {
          if (!ask) return;
          if (override) post("/api/board-override", { clear: true });
          go({ index: ask.index });
        }}
      />
      <BoardEditor
        kind={editing}
        svc={svc}
        slide={editing ? (state?.held && picks[editing]?.[0] === index && state.freshBoard) || picks[editing]?.[1] : undefined}
        onScreen={!!editing && current === editing}
        onClose={() => setEditing(null)}
      />
    </aside>
  );
}
