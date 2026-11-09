import type { Service, View } from "@/lib/api";
import { go, post } from "@/lib/api";
import type { Alarm } from "@/lib/problems";

// On a phone the band grows with its words, and the buttons go under them when they do not fit.
const band = "flex h-20 shrink-0 items-center gap-5 px-6 max-md:h-auto max-md:min-h-16 max-md:flex-wrap max-md:gap-x-3 max-md:gap-y-2 max-md:px-3 max-md:py-2";
const bandButton = "h-[72px] shrink-0 rounded-xl px-7 text-[26px] font-semibold transition active:scale-[0.98] max-md:h-14 max-md:px-4 max-md:text-[20px]";
const bandText = "min-w-0 flex-1 truncate max-md:min-w-[9rem] max-md:leading-tight max-md:whitespace-normal";

/** The one alarm: a red band under the top bar, a few words and one button. */
export function AlarmBand({ alarm, onAct }: { alarm: Alarm; onAct: (a: Alarm) => void }) {
  return (
    <div role="alert" className={`${band} bg-live text-white`}>
      <span className="size-4 shrink-0 animate-pulse-dot rounded-full bg-white" />
      <span className={`${bandText} text-[30px] font-bold max-md:text-[22px]`}>{alarm.title}</span>
      <button className={`${bandButton} bg-white text-live hover:bg-[#fff1f0]`} onClick={() => onAct(alarm)}>{alarm.button}</button>
    </div>
  );
}

/** What the operator chose for the boards, in one calm line; shares its place with the alarm. */
export function Notice({ state, svc, live }: { state: View | null; svc?: Service; live: boolean }) {
  if (state?.black)
    return (
      <div className={`${band} border-b-2 border-walnut/30 bg-[#e8dcc0]`}>
        <span className={`${bandText} text-[24px] font-semibold max-md:text-[20px]`}>Borden zwart</span>
        <button className={`${bandButton} bg-walnut text-white hover:bg-[#6b4a33]`} onClick={() => go({ kind: "zwart" })}>Weer aan</button>
      </div>
    );
  if (state?.overrideBoard)
    return (
      <div className={`${band} border-b-2 border-walnut/30 bg-[#e8dcc0]`}>
        <span className={`${bandText} text-[24px] font-semibold max-md:text-[20px]`}>Bord toont eigen tekst</span>
        <button className={`${bandButton} bg-walnut text-white hover:bg-[#6b4a33]`} onClick={() => post("/api/board-override", { clear: true })}>
          Terug naar normaal
        </button>
      </div>
    );
  if (state?.held)
    return (
      <div className={`${band} border-b-2 border-walnut/30 bg-[#e8dcc0]`}>
        <span className={`${bandText} text-[24px] font-semibold max-md:text-[20px]`}>Scherm toont nog de vorige versie</span>
        <button className={`${bandButton} bg-walnut text-white hover:bg-[#6b4a33]`} onClick={() => post("/api/tonen")}>Nu tonen</button>
      </div>
    );
  // During the service it waits in the website popover; no news in the band then.
  if (svc?.websiteChanged && !live)
    return (
      <div className={`${band} border-b-2 border-warn/40 bg-[#f6e6c2]`}>
        <span className={`${bandText} text-[24px] font-semibold max-md:text-[20px]`}>Website gewijzigd</span>
        <button className={`${bandButton} bg-walnut text-white hover:bg-[#6b4a33]`} onClick={() => post("/api/website", { id: svc.id, take: true })}>Overnemen</button>
        <button className={`${bandButton} border-2 border-input bg-card hover:bg-accent`} onClick={() => post("/api/website", { id: svc.id, take: false })}>Negeren</button>
      </div>
    );
  return null;
}
