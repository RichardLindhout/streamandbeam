import { CircleAlert, EyeOff, Type, WifiOff } from "lucide-react";
import type { StreamStatus, View, YouTubeCheck } from "@/lib/api";
import { go, post } from "@/lib/api";
import { Button } from "./ui/button";
import { cn } from "@/lib/utils";
import type { ReactNode } from "react";

function Bar({ tone, icon, children, action }: { tone: "red" | "grey" | "amber"; icon: ReactNode; children: ReactNode; action?: ReactNode }) {
  return (
    <div
      className={cn(
        "flex min-h-14 items-center justify-center gap-3 px-5 py-2.5 text-center text-[17px] font-bold [&_svg]:size-6",
        tone === "red" && "animate-blink bg-live text-white",
        tone === "grey" && "bg-[#2a323d] text-white",
        tone === "amber" && "bg-warn text-black",
      )}
    >
      {icon}
      <span>{children}</span>
      {action}
    </div>
  );
}

/** Bars across the top for anything the operator must notice right now. */
export function Alarms({ state, status, yt, connected }: { state: View | null; status: StreamStatus; yt: YouTubeCheck | null; connected: boolean }) {
  return (
    <div className="sticky top-0 z-40">
      {!connected && <Bar tone="grey" icon={<WifiOff />}>Geen verbinding met de kerk-PC — controleer of het programma draait en het netwerk werkt</Bar>}
      {status.audioWarning && <Bar tone="red" icon={<CircleAlert />}>{status.audioWarning}</Bar>}
      {status.mode === "live" && yt?.at && yt.live && !yt.audio ? (
        <Bar tone="red" icon={<CircleAlert />}>Op YouTube is geen geluid te horen — {yt.problem}</Bar>
      ) : null}
      {state?.overrideBoard && (
        <Bar
          tone="amber"
          icon={<Type />}
          action={<Button size="sm" variant="secondary" className="ml-2 bg-black/85 text-white hover:bg-black" onClick={() => post("/api/board-override", { clear: true })}>Terug naar normaal</Button>}
        >
          Het bord toont nu je vrije tekst
        </Bar>
      )}
      {state?.black && (
        <Bar tone="grey" icon={<EyeOff />} action={<Button size="sm" className="ml-2" onClick={() => go({ kind: "zwart" })}>Weer aan</Button>}>
          De borden zijn ZWART
        </Bar>
      )}
    </div>
  );
}
