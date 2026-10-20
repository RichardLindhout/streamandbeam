import { useEffect, useRef, useState } from "react";
import { setLiveConfirm } from "@/lib/api";
import {
  AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle,
} from "./ui/alert-dialog";

/** The one question before an action viewers would see while live. */
export function LiveConfirm() {
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
            <span className="mt-2 block">Kijkers zien dit direct.</span>
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
