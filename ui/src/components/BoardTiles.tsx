import { X } from "lucide-react";
import type { Slide, View } from "@/lib/api";
import { go, post } from "@/lib/api";
import { cn } from "@/lib/utils";
import { Board } from "./Board";

const KINDS = [
  { kind: "bord", label: "Liturgie" },
  { kind: "thema", label: "Thema" },
  { kind: "slotzang", label: "Slotzang" },
] as const;

/** The slide of this kind nearest after the current one (else the first), with its index. */
function pick(boards: Slide[], kind: string, index: number): [number, Slide] | null {
  if (boards[index]?.kind === kind) return [index, boards[index]];
  for (let i = index + 1; i < boards.length; i++) if (boards[i].kind === kind) return [i, boards[i]];
  const i = boards.findIndex((b) => b.kind === kind);
  return i >= 0 ? [i, boards[i]] : null;
}

function Tile({ label, on, missing, onClick, children, extra }: {
  label: string; on: boolean; missing?: boolean; onClick?: () => void; children: React.ReactNode; extra?: React.ReactNode;
}) {
  return (
    <div className="relative flex min-w-0 flex-col gap-2">
      <button
        type="button"
        disabled={missing || on || !onClick}
        onClick={onClick}
        className={cn(
          "group relative rounded-xl transition disabled:cursor-default",
          on ? "ring-[4px] ring-primary shadow-[0_0_40px_-8px_var(--primary)]" : "ring-1 ring-[#3a3229] hover:ring-2 hover:ring-primary/50",
        )}
      >
        <div className={cn("overflow-hidden rounded-xl transition", !on && "opacity-45 saturate-50 group-hover:opacity-90", missing && "!opacity-20")}>{children}</div>
        {on && (
          <span className="absolute -top-2.5 left-1/2 -translate-x-1/2 hymnal rounded-md bg-primary px-2.5 py-0.5 text-[12px] whitespace-nowrap text-primary-foreground shadow-lg">
            OP HET BORD
          </span>
        )}
        {missing && <span className="absolute inset-0 grid place-items-center px-2 text-center text-[11px] font-semibold text-muted-foreground">niet in deze dienst</span>}
      </button>
      {extra}
      <div className={cn("hymnal truncate text-center text-[14px]", on ? "text-primary" : "text-muted-foreground")}>{label}</div>
    </div>
  );
}

/** The three boards the church can show, side by side; the one on the TVs is lit. */
export function BoardTiles({ state }: { state: View | null }) {
  const boards = state?.boards?.length ? state.boards : state?.slides || [];
  const index = state?.index ?? 0;
  const override = state?.overrideBoard;
  const current = override ? "vrij" : state?.black ? "zwart" : boards[index]?.kind;
  return (
    <div className={cn("grid items-start gap-3", override ? "grid-cols-4" : "grid-cols-3")}>
      {KINDS.map(({ kind, label }) => {
        const p = pick(boards, kind, index);
        return (
          <Tile key={kind} label={label} on={current === kind} missing={!p} onClick={p ? () => go({ index: p[0] }) : undefined}>
            <Board slide={p?.[1]} />
          </Tile>
        );
      })}
      {override && (
        <Tile
          label="Vrije tekst"
          on
          extra={
            <button
              type="button"
              title="Vrije tekst weghalen"
              onClick={() => post("/api/board-override", { clear: true })}
              className="absolute -top-2.5 -right-2.5 z-10 grid size-8 place-items-center rounded-full bg-primary text-primary-foreground shadow-lg hover:brightness-110"
            >
              <X className="size-4" strokeWidth={3} />
            </button>
          }
        >
          <Board slide={override} />
        </Tile>
      )}
    </div>
  );
}
