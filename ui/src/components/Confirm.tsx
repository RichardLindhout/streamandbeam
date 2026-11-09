import {
  AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle,
} from "./ui/alert-dialog";
import { cn } from "@/lib/utils";

const TONE = {
  live: "bg-live text-white hover:brightness-110",
  walnut: "bg-walnut text-white hover:bg-[#6b4a33]",
  gold: "bg-primary text-primary-foreground hover:bg-primary-hover",
};

/** One question, two big answers: yes on the left, no on the right. A `warning` stands out
 *  in amber above the answers. */
export function Confirm({ open, onOpenChange, title, text, warning, yes, no = "Nee", onYes, tone = "walnut" }: {
  open: boolean; onOpenChange: (b: boolean) => void; title: string; text?: string; warning?: string; yes: string; no?: string; onYes: () => void; tone?: keyof typeof TONE;
}) {
  return (
    <AlertDialog open={open} onOpenChange={onOpenChange}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>{title}</AlertDialogTitle>
          {text ? <AlertDialogDescription>{text}</AlertDialogDescription> : <AlertDialogDescription className="sr-only">{title}</AlertDialogDescription>}
          {warning && (
            <div role="note" className="mt-2 rounded-xl border-2 border-warn/60 bg-warn/15 px-4 py-3 text-[22px] leading-snug font-semibold text-[#5a3a00]">
              ⚠ {warning}
            </div>
          )}
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogAction className={cn("h-[88px] text-[26px]", TONE[tone])} onClick={onYes}>{yes}</AlertDialogAction>
          <AlertDialogCancel className="h-[88px] text-[26px]">{no}</AlertDialogCancel>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
