import type { Service } from "@/lib/api";
import { shortDate } from "@/lib/api";
import { cn } from "@/lib/utils";

/** Services in date order, newest last. */
export function sortedServices(list: Service[] | undefined) {
  return [...(list || [])].sort((a, b) => (a.date + a.time).localeCompare(b.date + b.time));
}

/** The special day from the church's calendar, unless the service is already called so. */
export function specialDay(s?: Service) {
  const d = s?.specialDay || "";
  return d && d.toLowerCase() !== (s?.label || "").toLowerCase() ? d : "";
}

/** One big tile per service: "Zo 25 okt · 09:30 Morgendienst", with the special day beside it.
 *  Any service can be picked, on any day. */
export function ServiceTiles({ services, active, live, onPick, className }: {
  services: Service[] | undefined; active?: string; live?: string; onPick: (s: Service) => void; className?: string;
}) {
  const list = sortedServices(services);
  if (!list.length) return <div className="p-4 text-[22px] text-muted-foreground">Geen diensten</div>;
  return (
    <div className={cn("grid gap-2", className)}>
      {list.map((s) => (
        <button
          key={s.id}
          type="button"
          onClick={() => onPick(s)}
          className={cn(
            "flex min-h-[72px] w-full items-center gap-4 rounded-xl border-2 px-5 py-3 text-left text-[26px] transition",
            s.id === active ? "border-walnut bg-[#efe3c6]" : "border-transparent bg-card hover:bg-accent",
          )}
        >
          <span className="min-w-0 flex-1 truncate">
            <span className="font-semibold tabular-nums">{shortDate(s)} · {s.time}</span> {s.label}
            {specialDay(s) && <span className="hymnal ml-3 text-[20px] text-[#7a5a1c]">{specialDay(s)}</span>}
          </span>
          {s.id === live && <span className="shrink-0 rounded-full bg-live px-3 py-1 text-[18px] font-bold text-white">LIVE</span>}
        </button>
      ))}
    </div>
  );
}
