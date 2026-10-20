import type { View } from "@/lib/api";
import { go, post } from "@/lib/api";
import type { Alarm } from "@/lib/problems";

const band = "flex h-20 shrink-0 items-center gap-5 px-6";
const bandButton = "h-[72px] shrink-0 rounded-xl px-7 text-[26px] font-semibold transition active:scale-[0.98]";

/** The one alarm: a red band under the top bar, a few words and one button. */
export function AlarmBand({ alarm, onAct }: { alarm: Alarm; onAct: (a: Alarm) => void }) {
  return (
    <div role="alert" className={`${band} bg-live text-white`}>
      <span className="size-4 shrink-0 animate-pulse-dot rounded-full bg-white" />
      <span className="min-w-0 flex-1 truncate text-[30px] font-bold">{alarm.title}</span>
      <button className={`${bandButton} bg-white text-live hover:bg-[#fff1f0]`} onClick={() => onAct(alarm)}>{alarm.button}</button>
    </div>
  );
}

/** What the operator chose for the boards, in one calm line; shares its place with the alarm. */
export function Notice({ state }: { state: View | null }) {
  if (state?.black)
    return (
      <div className={`${band} border-b-2 border-walnut/30 bg-[#e8dcc0]`}>
        <span className="min-w-0 flex-1 truncate text-[24px] font-semibold">Borden zwart</span>
        <button className={`${bandButton} bg-walnut text-white hover:bg-[#6b4a33]`} onClick={() => go({ kind: "zwart" })}>Weer aan</button>
      </div>
    );
  if (state?.overrideBoard)
    return (
      <div className={`${band} border-b-2 border-walnut/30 bg-[#e8dcc0]`}>
        <span className="min-w-0 flex-1 truncate text-[24px] font-semibold">Bord toont eigen tekst</span>
        <button className={`${bandButton} bg-walnut text-white hover:bg-[#6b4a33]`} onClick={() => post("/api/board-override", { clear: true })}>
          Terug naar normaal
        </button>
      </div>
    );
  return null;
}
