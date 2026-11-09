import { useEffect, useRef, useState } from "react";
import type { StreamStatus } from "@/lib/api";
import { setLiveConfirm, setStreamMode } from "@/lib/api";
import { Confirm } from "./Confirm";
import {
  AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle,
} from "./ui/alert-dialog";

/** Live without YouTube: what the volunteer needs to know before and during. */
export const NO_YOUTUBE = "Geen YouTube ingesteld: er gaat niets naar YouTube. Je ziet het live beeld hier en we nemen op (mp4 met beeld en geluid).";

/** "Live gaan?" Without YouTube set up it says plainly that only the recording runs. */
export function GoLive({ open, onOpenChange, status }: { open: boolean; onOpenChange: (b: boolean) => void; status: StreamStatus }) {
  const noYouTube = status.youtubeConfigured === false;
  return (
    <Confirm
      open={open}
      onOpenChange={onOpenChange}
      title={noYouTube ? "Live gaan zonder YouTube?" : "Live gaan?"}
      warning={noYouTube ? NO_YOUTUBE : undefined}
      yes="Ja, live"
      tone="live"
      onYes={() => setStreamMode("live")}
    />
  );
}

/** The one question before an action viewers would see while live. */
export function LiveConfirm({ status }: { status: StreamStatus }) {
  const [what, setWhat] = useState<string | null>(null);
  const answer = useRef<(yes: boolean) => void>(() => {});
  useEffect(() => {
    setLiveConfirm(
      (w) =>
        new Promise<boolean>((resolve) => {
          answer.current(false);
          answer.current = resolve;
          setWhat(w);
        }),
    );
  }, []);
  const done = (yes: boolean) => {
    answer.current(yes);
    answer.current = () => {};
    setWhat(null);
  };
  return (
    <AlertDialog open={what !== null} onOpenChange={(o) => !o && done(false)}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>Je bent live</AlertDialogTitle>
          <AlertDialogDescription className="text-[24px] text-foreground">
            <span className="block font-semibold">{what}</span>
            {/* Without YouTube nobody watches, but it goes into the recording. */}
            <span className="mt-2 block">{status.youtube === "geen" ? "Het komt direct in de opname." : "Kijkers zien dit direct."}</span>
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel className="h-[88px] text-[26px]" onClick={() => done(false)}>Annuleren</AlertDialogCancel>
          <AlertDialogAction className="h-[88px] bg-live text-[26px] text-white hover:brightness-110" onClick={() => done(true)}>Ja, doorgaan</AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
