import {
  BookText, BookOpen, Church, Coins, FileText, Flag, HandHeart, Info, Mic, EighthNote, EighthNotes, ScrollText, Circle, type LucideIcon,
} from "lucide-react";

export const KIND_ICON: Record<string, LucideIcon> = {
  welkom: Church,
  psalm: EighthNote,
  slotzang: EighthNote,
  nazang: EighthNote,
  lied: EighthNotes,
  lezing: BookOpen,
  catechismus: BookText,
  belijdenis: ScrollText,
  formulier: FileText,
  gebed: HandHeart,
  collecte: Coins,
  preek: Mic,
  mededeling: Info,
  einde: Flag,
};

export function KindIcon({ kind, className }: { kind: string; className?: string }) {
  const I = KIND_ICON[kind] || Circle;
  return <I className={className} />;
}
