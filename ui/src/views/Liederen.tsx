import { useEffect, useMemo, useState } from "react";
import { Plus, Search } from "lucide-react";
import { toast } from "sonner";
import type { Bundel, ListedSong, Song, StreamStatus } from "@/lib/api";
import { at, hhmm, post } from "@/lib/api";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { Confirm } from "@/components/Confirm";

const select = "h-14 w-full rounded-xl border-2 border-input bg-card px-3 text-[20px]";

/** What the form holds: the words as one text, a verse per block (an empty line between). */
type Draft = { oud: string; titel: string; bundel: string; nummer: string; namen: string; tekst: string; eigen: boolean; ingebouwd: boolean };

const kort = (bundels: Bundel[], naam?: string) => bundels.find((b) => b.naam === naam)?.kort ?? "";

/** "LB 23", else the title. */
function name(s: Song, bundels: Bundel[]) {
  const k = kort(bundels, s.bundel);
  return k && s.nummer ? `${k} ${s.nummer}` : s.titel;
}

function toDraft(s?: ListedSong): Draft {
  return {
    oud: s?.titel ?? "",
    titel: s?.titel ?? "",
    bundel: s?.bundel ?? "",
    nummer: s?.nummer ? String(s.nummer) : "",
    namen: (s?.aliassen ?? []).join("\n"),
    tekst: (s?.verzen ?? []).map((v) => v.join("\n")).join("\n\n"),
    eigen: !!s?.eigen,
    ingebouwd: !!s?.ingebouwd,
  };
}

/** The verses from the text: blocks between empty lines; a block that starts with only its
 *  number ("1" or "1.") loses that line. */
function verses(text: string): string[][] {
  return text
    .replace(/\r/g, "")
    .split(/\n\s*\n/)
    .map((block) => block.split("\n").map((l) => l.trim()).filter(Boolean))
    .map((lines) => (lines.length > 1 && /^\d+[.)]?$/.test(lines[0]) ? lines.slice(1) : lines))
    .filter((lines) => lines.length > 0);
}

/** The songs that are not in the psalm book: the church's own, with the song book and number
 *  they are sung by ("LB 23"), and the words it may show under its own licence. */
export function Liederen({ status, onBack }: { status: StreamStatus; onBack: () => void }) {
  const [list, setList] = useState<ListedSong[]>([]);
  const [bundels, setBundels] = useState<Bundel[]>([]);
  const [find, setFind] = useState("");
  const [draft, setDraft] = useState<Draft | null>(null);
  const [dirty, setDirty] = useState(false);
  const [ask, setAsk] = useState<null | { title: string; yes: string; run: () => void }>(null);
  const live = status.mode === "live";

  useEffect(() => {
    fetch("/api/liederen")
      .then((r) => r.json())
      .then((v: { liederen: ListedSong[]; bundels: Bundel[] }) => {
        setList(v.liederen);
        setBundels(v.bundels);
      })
      .catch(() => toast.error("Geen verbinding met de kerk-PC"));
  }, []);

  const shown = useMemo(() => {
    const q = find.trim().toLowerCase();
    if (!q) return list;
    return list.filter((s) => [s.titel, name(s, bundels), ...(s.aliassen ?? [])].some((t) => t.toLowerCase().includes(q)));
  }, [list, bundels, find]);

  const leave = (run: () => void) => (dirty ? setAsk({ title: "Wijzigingen weggooien?", yes: "Weggooien", run }) : run());
  const open = (d: Draft | null) => {
    setDraft(d);
    setDirty(false);
  };
  const change = (patch: Partial<Draft>) => {
    setDraft((d) => (d ? { ...d, ...patch } : d));
    setDirty(true);
  };

  async function save() {
    if (!draft) return;
    const nummer = draft.bundel ? Math.max(0, Math.round(+draft.nummer || 0)) : 0;
    if (!draft.titel.trim() && !(draft.bundel && nummer)) return void toast.error("Geef het lied een titel of een nummer in een bundel");
    const lied: Song = {
      titel: draft.titel.trim(),
      bundel: draft.bundel,
      nummer,
      aliassen: draft.namen.split("\n").map((n) => n.trim()).filter(Boolean),
      verzen: verses(draft.tekst),
    };
    const r = await post("/api/lied", { oud: draft.oud, lied });
    if (!r.ok) return;
    const v: { liederen: ListedSong[] } = await r.json();
    setList(v.liederen);
    // The saved song, found again by its title (a song in a book without one is called "LB 23").
    const title = lied.titel || `${kort(bundels, lied.bundel)} ${nummer}`;
    open(toDraft(v.liederen.find((s) => s.eigen && s.titel.toLowerCase() === title.toLowerCase())));
    toast.success("Opgeslagen");
  }

  async function remove() {
    if (!draft?.eigen) return;
    const r = await post("/api/lied-weg", { titel: draft.oud });
    if (!r.ok) return;
    const v: { liederen: ListedSong[] } = await r.json();
    setList(v.liederen);
    // A built-in song is back as it was; an own one is gone.
    const back = draft.ingebouwd ? v.liederen.find((s) => s.titel.toLowerCase() === draft.oud.toLowerCase()) : undefined;
    open(back ? toDraft(back) : null);
  }

  const n = verses(draft?.tekst ?? "").length;
  const how = draft && (draft.bundel && draft.nummer ? `${kort(bundels, draft.bundel)} ${draft.nummer}:1` : draft.titel.trim() ? `lied: ${draft.titel.trim()}` : "");

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex h-24 shrink-0 items-center gap-5 border-b-2 border-walnut/15 bg-[#efe5cc] px-4">
        <Button variant="outline" onClick={() => leave(onBack)} className="text-[24px]">← Terug naar de dienst</Button>
        <h1 className="min-w-0 flex-1 truncate font-serif text-[30px] font-semibold">Liederen</h1>
        {live && (
          <span className="flex h-14 shrink-0 items-center gap-2.5 rounded-full bg-live px-5 text-[22px] font-bold text-white tabular-nums">
            <span className="size-3 animate-pulse-dot rounded-full bg-white" /> LIVE {hhmm(at(status.since))}
          </span>
        )}
      </div>

      <div className="flex min-h-0 flex-1 max-md:flex-col">
        <aside className={cn("scroll-thin flex w-[400px] shrink-0 flex-col gap-2 [&>*]:shrink-0 overflow-y-auto border-r-2 border-walnut/15 bg-[#efe5cc]/60 p-3 max-md:w-full max-md:border-r-0", draft && "max-md:hidden")}>
          <Button size="lg" className="w-full" onClick={() => leave(() => { setDraft(toDraft()); setDirty(true); })}><Plus /> Nieuw lied</Button>
          <label className="relative">
            <Search className="absolute top-1/2 left-3 size-5 -translate-y-1/2 text-muted-foreground" />
            <Input value={find} onChange={(e) => setFind(e.target.value)} placeholder="Zoeken" className="h-14 pl-11 text-[20px]" />
          </label>
          {shown.map((s) => {
            const nm = name(s, bundels);
            const on = draft?.oud === s.titel && !!draft.oud;
            return (
              <button
                key={`${s.eigen}-${s.titel}`}
                onClick={() => !on && leave(() => open(toDraft(s)))}
                className={cn("grid gap-0.5 rounded-xl border-2 px-3 py-2 text-left transition", on ? "border-primary bg-[#fdf8ec]" : "border-transparent hover:bg-accent")}
              >
                <span className="truncate text-[21px] font-semibold">{nm}</span>
                <span className="flex flex-wrap items-baseline gap-x-2 text-[17px] text-muted-foreground">
                  {nm !== s.titel && <span className="truncate">{s.titel}</span>}
                  {!s.tekst && <span className="rounded bg-warn/20 px-1.5 text-[#7a4a00]">geen tekst</span>}
                  {s.eigen && <span className="rounded bg-[#fdf8ec] px-1.5 text-[#7a5a1c]">eigen</span>}
                </span>
              </button>
            );
          })}
          {!shown.length && <p className="px-2 py-4 text-[20px] text-muted-foreground">Geen lied gevonden.</p>}
        </aside>

        {draft ? (
          <main className="scroll-thin min-h-0 flex-1 overflow-y-auto p-5">
            <div className="mx-auto grid max-w-3xl gap-5">
              <Button variant="outline" className="md:hidden" onClick={() => leave(() => open(null))}>← Alle liederen</Button>
              <div className="grid gap-4 sm:grid-cols-[2fr_1fr]">
                <label className="grid gap-1.5 text-[18px] text-muted-foreground">
                  Bundel
                  <select className={select} value={draft.bundel} onChange={(e) => change({ bundel: e.target.value })}>
                    <option value="">Geen bundel</option>
                    {bundels.map((b) => <option key={b.naam} value={b.naam}>{b.naam} ({b.kort})</option>)}
                  </select>
                </label>
                {draft.bundel && (
                  <label className="grid gap-1.5 text-[18px] text-muted-foreground">
                    Nummer
                    <Input type="number" min={1} inputMode="numeric" value={draft.nummer} onChange={(e) => change({ nummer: e.target.value })} className="h-14 text-[20px]" />
                  </label>
                )}
              </div>
              <label className="grid gap-1.5 text-[18px] text-muted-foreground">
                Titel
                <Input value={draft.titel} onChange={(e) => change({ titel: e.target.value })} placeholder="Bijvoorbeeld: Heer, U bent mijn leven" className="h-14 text-[20px]" />
              </label>
              <label className="grid gap-1.5 text-[18px] text-muted-foreground">
                Ook bekend als (één per regel)
                <Textarea rows={2} value={draft.namen} onChange={(e) => change({ namen: e.target.value })} className="text-[20px]" />
              </label>
              <label className="grid gap-1.5 text-[18px] text-muted-foreground">
                <span>Tekst — een lege regel tussen de verzen{n > 0 && <span className="text-foreground"> · {n} {n === 1 ? "vers" : "verzen"}</span>}</span>
                <Textarea rows={14} value={draft.tekst} onChange={(e) => change({ tekst: e.target.value })} className="font-serif text-[21px] leading-snug" />
              </label>
              <p className="rounded-xl border-2 border-warn/40 bg-warn/10 px-4 py-3 text-[18px] leading-snug text-[#5a3f12]">
                Teksten uit het Liedboek, Weerklank, Opwekking en andere bundels zijn beschermd. Zet ze hier alleen in als jullie kerk er een licentie voor
                heeft (bijvoorbeeld Licentie Kerkmuziek of CCLI). Ze blijven op deze computer.
              </p>
              {how && <p className="text-[18px] text-muted-foreground">In de dienst: typ <b className="text-foreground">{how}</b> bij het invullen van de liturgie.</p>}
              <div className="flex flex-wrap items-center gap-3">
                <Button size="lg" disabled={!dirty} onClick={save} className="px-10">Opslaan</Button>
                {draft.eigen && (
                  <Button
                    variant="outline"
                    size="lg"
                    className={cn(!draft.ingebouwd && "text-live")}
                    onClick={() => setAsk({ title: draft.ingebouwd ? "Terug naar de ingebouwde versie?" : `${draft.oud} verwijderen?`, yes: draft.ingebouwd ? "Ja" : "Verwijderen", run: remove })}
                  >
                    {draft.ingebouwd ? "Terug naar ingebouwd" : "Verwijderen"}
                  </Button>
                )}
              </div>
            </div>
          </main>
        ) : (
          <div className="grid flex-1 place-items-center p-8 text-center text-[24px] text-muted-foreground max-md:hidden">
            Kies links een lied, of maak een nieuw.
          </div>
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
