import { useState } from "react";
import { Ellipsis, Globe, Trash } from "lucide-react";
import { toast } from "sonner";
import type { StreamStatus, View } from "@/lib/api";
import { at, hhmm, post } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { DropdownMenu, DropdownMenuContent, DropdownMenuItem, DropdownMenuTrigger } from "@/components/ui/dropdown-menu";
import { OrderEditor } from "@/components/OrderEditor";
import { SermonCard } from "@/components/PrepareCards";
import { ServiceTiles } from "@/components/ServiceTiles";
import { Confirm } from "@/components/Confirm";

export function Voorbereiden({ state, status, onBack }: { state: View | null; status: StreamStatus; onBack: () => void }) {
  const [openId, setOpenId] = useState<string | null>(null);
  const [ask, setAsk] = useState<"" | "fetch" | "delete">("");
  const live = status.mode === "live";
  const svc = state?.services?.find((s) => s.id === openId) || state?.services?.find((s) => s.id === state.current);
  const isLive = live && !!svc && svc.id === state?.current;

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
            <DropdownMenuItem disabled={live || !svc} onSelect={() => setAsk("fetch")}><Globe /> Opnieuw ophalen</DropdownMenuItem>
            <DropdownMenuItem disabled={isLive || !svc} onSelect={() => setAsk("delete")} className="text-live [&_svg]:text-live"><Trash /> Dienst verwijderen</DropdownMenuItem>
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
          <div className="grid items-start gap-5 lg:grid-cols-[minmax(0,1.6fr)_minmax(340px,1fr)]">
            {svc ? (
              <OrderEditor key={svc.id} svc={svc} state={state} />
            ) : (
              <Card className="grid place-items-center p-12 text-[26px] text-muted-foreground">Geen dienst</Card>
            )}
            {svc && <SermonCard key={svc.id} svc={svc} />}
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
    </div>
  );
}
