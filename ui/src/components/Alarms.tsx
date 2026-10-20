import { useState } from "react";
import { Bell, BellOff, CircleHelp, EyeOff, Type, WifiOff } from "lucide-react";
import type { View } from "@/lib/api";
import { go, post } from "@/lib/api";
import type { Problem } from "@/lib/problems";
import { cn } from "@/lib/utils";
import { Button } from "./ui/button";
import { Dialog, DialogBody, DialogContent, DialogDescription, DialogHeader, DialogTitle } from "./ui/dialog";

/** What to do about a problem: a short checklist, and the technical detail at the bottom. */
function Steps({ p, open, onOpenChange }: { p: Problem; open: boolean; onOpenChange: (b: boolean) => void }) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{p.title}</DialogTitle>
          <DialogDescription>{p.text}</DialogDescription>
        </DialogHeader>
        <DialogBody className="gap-5">
          <ol className="grid gap-3">
            {p.steps.map((s, i) => (
              <li key={i} className="flex gap-3.5 text-[16px] leading-snug">
                <span className="grid size-7 shrink-0 place-items-center rounded-full bg-primary font-serif text-sm font-semibold text-primary-foreground">{i + 1}</span>
                <span className="pt-0.5">{s}</span>
              </li>
            ))}
          </ol>
          {p.detail && (
            <div className="rounded-lg border bg-background/50 p-3 text-xs text-muted-foreground">
              <div className="mb-1 font-semibold">Voor de technicus</div>
              {p.detail}
            </div>
          )}
        </DialogBody>
      </DialogContent>
    </Dialog>
  );
}

/** One big, unmistakable block when something is wrong. Nothing at all when all is well. */
export function AlarmBlock({ problems, muted, setMuted }: { problems: Problem[]; muted: boolean; setMuted: (b: boolean) => void }) {
  const [help, setHelp] = useState(false);
  // Only one alarm at a time, and only the serious ones; the rest is in the status light.
  const p = problems.find((x) => x.severity === "rood");
  if (!p) return null;
  const red = p.severity === "rood";
  const Icon = p.id === "verbinding" ? WifiOff : Bell;
  return (
    <div className="px-5 pt-5">
      <div
        className={cn(
          "flex flex-wrap items-center gap-5 rounded-2xl border-2 px-6 py-5 animate-alarm",
          red ? "border-live bg-[#3a1418] [--alarm:rgb(229_52_63/0.45)]" : "border-warn bg-[#33250f] [--alarm:rgb(217_150_43/0.4)]",
        )}
      >
        <span className={cn("grid size-16 shrink-0 place-items-center rounded-2xl", red ? "bg-live text-[#fff6ef]" : "bg-warn text-[#1b150d]")}>
          <Icon className="size-9 animate-ring" strokeWidth={2.2} />
        </span>
        <div className="min-w-0 flex-1">
          <div className={cn("font-serif text-[30px] leading-tight font-semibold uppercase tracking-wide", red ? "text-[#ffd9db]" : "text-[#ffe1ad]")}>{p.title}</div>
          <div className="text-[18px] text-foreground/90">
            {p.text}
          </div>
        </div>
        <Button size="xl" className="h-14 rounded-xl bg-[#efe6d6] px-6 text-[17px] text-[#1b150d] hover:bg-white" onClick={() => setHelp(true)}>
          <CircleHelp className="size-5" /> Wat moet ik doen?
        </Button>
        <button
          onClick={() => setMuted(!muted)}
          title={muted ? "Waarschuwingstoon aan" : "Waarschuwingstoon uit"}
          className="grid size-12 place-items-center rounded-xl text-foreground/70 transition hover:bg-white/10 hover:text-foreground"
        >
          {muted ? <BellOff className="size-5" /> : <Bell className="size-5" />}
        </button>
      </div>
      <Steps p={p} open={help} onOpenChange={setHelp} />
    </div>
  );
}

/** Calm notices for things the operator chose: free text on the board, black boards. */
export function Notices({ state }: { state: View | null }) {
  if (!state?.overrideBoard && !state?.black) return null;
  return (
    <div className="grid gap-3 px-5 pt-5">
      {state.overrideBoard && (
        <div className="flex flex-wrap items-center gap-4 rounded-2xl border border-primary/40 bg-primary/10 px-5 py-3.5">
          <Type className="size-5 text-primary" />
          <span className="flex-1 text-[16px]">Het bord toont nu je eigen tekst.</span>
          <Button variant="outline" onClick={() => post("/api/board-override", { clear: true })}>Terug naar normaal</Button>
        </div>
      )}
      {state.black && (
        <div className="flex flex-wrap items-center gap-4 rounded-2xl border border-primary/40 bg-primary/10 px-5 py-3.5">
          <EyeOff className="size-5 text-primary" />
          <span className="flex-1 text-[16px]">De borden zijn zwart.</span>
          <Button onClick={() => go({ kind: "zwart" })}>Weer aan</Button>
        </div>
      )}
    </div>
  );
}
