import { useState } from "react";
import { Radio, Square } from "lucide-react";
import { toast } from "sonner";
import type { Mode, StreamStatus } from "@/lib/api";
import { clock, post } from "@/lib/api";
import {
  AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle,
} from "./ui/alert-dialog";
import { cn } from "@/lib/utils";

export async function setStreamMode(m: Mode) {
  const r = await post("/api/stream", { mode: m });
  if (!r.ok) toast.error("Lukt niet: " + (await r.text()));
}

/** Asks first, then switches the stream: going live and stopping a live stream. */
export function ConfirmMode({ mode, open, onOpenChange }: { mode: Mode | null; open: boolean; onOpenChange: (b: boolean) => void }) {
  const live = mode === "live";
  return (
    <AlertDialog open={open} onOpenChange={onOpenChange}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <div className={cn("mb-2 grid size-12 place-items-center rounded-xl", live ? "bg-live/15 text-live" : "bg-white/10 text-foreground")}>
            {live ? <Radio className="size-6" /> : <Square className="size-6" />}
          </div>
          <AlertDialogTitle>{live ? "Live gaan op YouTube?" : "Uitzending stoppen?"}</AlertDialogTitle>
          <AlertDialogDescription>
            {live
              ? "De kerkdienst gaat dan direct naar YouTube. Iedereen die kijkt, ziet en hoort wat de camera opneemt."
              : "Kijkers op YouTube zien dan niets meer. Doe dit pas als de dienst echt is afgelopen."}
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>Annuleer</AlertDialogCancel>
          <AlertDialogAction className={live ? "bg-live text-white" : "bg-foreground text-background"} onClick={() => mode && setStreamMode(mode)}>
            {live ? "Ja, ga live" : "Ja, stop"}
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}

/** The one button the volunteer needs for the stream: go live, or stop. */
export function LiveButton({ status }: { status: StreamStatus }) {
  const [ask, setAsk] = useState<Mode | null>(null);
  const live = status.mode === "live";
  return (
    <>
      {live ? (
        <button
          onClick={() => setAsk("stop")}
          className="group flex h-20 w-full items-center gap-4 rounded-2xl border-2 border-live bg-live/10 px-5 text-left transition hover:bg-live/20"
        >
          <span className="grid size-11 place-items-center rounded-xl bg-live text-white"><Square className="size-5 fill-current" /></span>
          <span className="leading-tight">
            <span className="block text-lg font-bold">Stop uitzending</span>
            <span className="font-mono text-sm text-[#ff9aa4]">live sinds {clock(status.since)}</span>
          </span>
        </button>
      ) : (
        <button
          onClick={() => setAsk("live")}
          className="flex h-20 w-full items-center gap-4 rounded-2xl bg-gradient-to-b from-[#ff4656] to-[#d81a2c] px-5 text-left text-white shadow-[0_14px_36px_-12px_rgb(255_45_63/0.85),inset_0_1px_0_rgb(255_255_255/0.25)] transition hover:brightness-110 active:scale-[0.99]"
        >
          <span className="grid size-11 place-items-center rounded-xl bg-white/20"><Radio className="size-6" /></span>
          <span className="leading-tight">
            <span className="block text-lg font-bold">Ga live</span>
            <span className="text-sm whitespace-nowrap text-white/80">naar YouTube</span>
          </span>
        </button>
      )}
      <ConfirmMode mode={ask} open={ask !== null} onOpenChange={(o) => !o && setAsk(null)} />
    </>
  );
}
