import {
  BookText, BookOpen, Church, Coins, FileText, Flag, Hand, HandHeart, Info, Mic, EighthNote, EighthNotes, ScrollText, Circle, type LucideIcon,
} from "lucide-react";
import type { Item } from "./api";

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
  zegen: Hand,
  einde: Flag,
};

/** Fixed names for the parts the program makes itself. */
const KIND_LABEL: Record<string, string> = { welkom: "Startscherm", einde: "Aftiteling", zegen: "Zegen" };

/** Sung parts: their slides are verses. */
export const SUNG = new Set(["psalm", "slotzang", "nazang", "lied"]);

export function itemLabel(it?: Item) {
  if (!it) return "";
  if (it.kind === "welkom" || it.kind === "einde") return KIND_LABEL[it.kind];
  return it.label || KIND_LABEL[it.kind] || it.kind;
}

/** The short reference beside the name ("Psalm 42 : 1, 3"), unless it only repeats the name. */
export function itemDetail(it?: Item) {
  const r = (it?.ref || "").trim();
  return r && r !== itemLabel(it) ? r : "";
}

export function KindIcon({ kind, className }: { kind: string; className?: string }) {
  const I = KIND_ICON[kind] || Circle;
  return <I className={className} />;
}
