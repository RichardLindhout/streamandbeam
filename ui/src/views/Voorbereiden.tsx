import { useState } from "react";
import { CalendarPlus, Copy, Ellipsis, Globe, MonitorPlay, Trash } from "lucide-react";
import { toast } from "sonner";
import type { StreamStatus, View } from "@/lib/api";
import { at, hhmm, longDate, post } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuSeparator, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { OrderEditor } from "@/components/OrderEditor";
import { SermonCard } from "@/components/PrepareCards";
import { ServiceTiles, specialDay } from "@/components/ServiceTiles";
import { NewServiceDialog } from "@/components/Dialogs";
import { Confirm } from "@/components/Confirm";

export function Voorbereiden({ state, status, onBack }: { state: View | null; status: StreamStatus; onBack: () => void }) {
  const [openId, setOpenId] = useState<string | null>(null);
  const [ask, setAsk] = useState<"" | "fetch" | "delete" | "select">("");
  // The new-service dialog, empty or with the liturgy of the open service.
  const [make, setMake] = useState<{ copy: string } | null>(null);
  const live = status.mode === "live";
  const svc = state?.services?.find((s) => s.id === openId) || state?.services?.find((s) => s.id === state.current);
  const isLive = live && !!svc && svc.id === state?.current;
  const onScreen = !!svc && svc.id === state?.current;
  const day = specialDay(svc);

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="flex h-24 shrink-0 items-center gap-5 border-b-2 border-walnut/15 bg-[#efe5cc] px-4">
        <Button variant="outline" onClick={onBack} className="text-[24px]">← Terug naar de dienst</Button>
        <h1 className="min-w-0 flex-1 truncate font-serif text-[30px] font-semibold">Dienst voorbereiden</h1>
        {live && (
          <span className="flex h-14 shrink-0 items-center gap-2.5 rounded-full bg-live px-5 text-[22px] font-bold text-white tabular-nums">
            <span className="size-3 animate-pulse-dot rounded-full bg-white" /> LIVE {hhmm(at(status.since))}
          </span>
        )}
        <DropdownMenu modal={false}>
          <DropdownMenuTrigger asChild>
            <Button variant="outline" size="icon" aria-label="Menu" className="shrink-0"><Ellipsis className="size-9" /></Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent>
            <DropdownMenuItem onSelect={() => setMake({ copy: "" })}><CalendarPlus /> Nieuwe dienst…</DropdownMenuItem>
            <DropdownMenuItem disabled={!svc} onSelect={() => svc && setMake({ copy: svc.id })}><Copy /> Kopie van deze dienst…</DropdownMenuItem>
            <DropdownMenuSeparator />
            <DropdownMenuItem disabled={!svc} onSelect={() => setAsk("fetch")}><Globe /> Opnieuw ophalen</DropdownMenuItem>
            <DropdownMenuItem disabled={!svc} onSelect={() => setAsk("delete")} className="text-live [&_svg]:text-live"><Trash /> Dienst verwijderen</DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
      </div>

      <main className="scroll-thin min-h-0 flex-1 overflow-y-auto">
        <div className="mx-auto grid w-full max-w-[1500px] content-start gap-5 p-5">
          <ServiceTiles
            services={state?.services}
            active={svc?.id}
            live={live ? state?.current : undefined}
            onPick={(s) => setOpenId(s.id)}
            className="grid-cols-[repeat(auto-fill,minmax(470px,1fr))]"
          />
          {isLive && <div className="rounded-xl bg-warn px-6 py-4 text-[26px] font-bold text-[#2b1d12]">Deze dienst is nu live</div>}
          {svc && (day || svc.operator || !onScreen) && (
            <div className="flex flex-wrap items-center gap-x-8 gap-y-3 rounded-xl border-2 bg-card px-5 py-3 text-[22px]">
              {day && <span className="hymnal text-[24px] text-[#7a5a1c]">{day}</span>}
              {svc.operator && <span>Bediening: <b>{svc.operator}</b></span>}
              {!onScreen && (
                <>
                  <span className="min-w-0 flex-1 text-muted-foreground">Op het scherm staat een andere dienst</span>
                  <Button onClick={() => setAsk("select")}><MonitorPlay /> Deze dienst op het scherm</Button>
                </>
              )}
            </div>
          )}
          {/* Each its own key: with the same key for both, picking another service left the
              order of the previous one standing beside the new one. */}
          <div className="grid items-start gap-5 lg:grid-cols-[minmax(0,1.6fr)_minmax(340px,1fr)]">
            {svc ? (
              <OrderEditor key={"volgorde " + svc.id} svc={svc} state={state} />
            ) : (
              <Card className="grid place-items-center p-12 text-[26px] text-muted-foreground">Geen dienst</Card>
            )}
            {svc && <SermonCard key={"preek " + svc.id} svc={svc} />}
          </div>
        </div>
      </main>

      <Confirm
        open={ask === "fetch"}
        onOpenChange={(o) => !o && setAsk("")}
        title="Opnieuw ophalen van de website?"
        yes="Ja, ophalen"
        onYes={async () => {
          const r = await post("/api/fetch");
          if (r.ok) toast.success("Opgehaald");
        }}
      />
      <Confirm
        open={ask === "delete"}
        onOpenChange={(o) => !o && setAsk("")}
        title={svc ? `${svc.label} verwijderen?` : ""}
        yes="Verwijderen"
        tone="live"
        onYes={async () => {
          if (!svc) return;
          const r = await post("/api/delete-service?id=" + encodeURIComponent(svc.id));
          if (r.ok) setOpenId(null);
        }}
      />
      {/* Any service can go on the screens, on any day (asked again while live). */}
      <Confirm
        open={ask === "select"}
        onOpenChange={(o) => !o && setAsk("")}
        title={svc ? `${svc.label} van ${longDate(svc).toLowerCase()} op het scherm?` : ""}
        yes="Ja"
        onYes={async () => {
          if (!svc) return;
          const r = await post("/api/select?id=" + encodeURIComponent(svc.id));
          if (r.ok) toast.success("Op het scherm");
        }}
      />
      <NewServiceDialog
        open={make !== null}
        onOpenChange={(o) => !o && setMake(null)}
        services={state?.services}
        copyFrom={make?.copy}
        onMade={(id) => setOpenId(id)}
      />
    </div>
  );
}
