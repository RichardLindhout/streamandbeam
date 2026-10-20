import {
  AlertDialog, AlertDialogAction, AlertDialogCancel, AlertDialogContent, AlertDialogDescription, AlertDialogFooter, AlertDialogHeader, AlertDialogTitle,
} from "./ui/alert-dialog";
import { cn } from "@/lib/utils";

const TONE = {
  live: "bg-live text-white hover:brightness-110",
  walnut: "bg-walnut text-white hover:bg-[#6b4a33]",
  gold: "bg-primary text-primary-foreground hover:bg-primary-hover",
};

/** One question, two big answers: yes on the left, no on the right. */
export function Confirm({ open, onOpenChange, title, text, yes, no = "Nee", onYes, tone = "walnut" }: {
  open: boolean; onOpenChange: (b: boolean) => void; title: string; text?: string; yes: string; no?: string; onYes: () => void; tone?: keyof typeof TONE;
}) {
  return (
    <AlertDialog open={open} onOpenChange={onOpenChange}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>{title}</AlertDialogTitle>
          {text ? <AlertDialogDescription>{text}</AlertDialogDescription> : <AlertDialogDescription className="sr-only">{title}</AlertDialogDescription>}
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogAction className={cn("h-[88px] text-[26px]", TONE[tone])} onClick={onYes}>{yes}</AlertDialogAction>
          <AlertDialogCancel className="h-[88px] text-[26px]">{no}</AlertDialogCancel>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
