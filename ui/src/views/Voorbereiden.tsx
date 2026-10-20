import { useState } from "react";
import { Eye, EyeOff, Images, Settings2, Zap } from "lucide-react";
import type { Service, StreamStatus, View } from "@/lib/api";
import { go } from "@/lib/api";
import { cn } from "@/lib/utils";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader } from "@/components/ui/card";
import { OrderEditor } from "@/components/OrderEditor";
import { FreeTextCard, SermonCard, StreamCard } from "@/components/PrepareCards";
import { SettingsDialog } from "@/components/Dialogs";
import { Strip } from "@/components/Strip";

function QuickJumps({ state }: { state: View | null }) {
  const jumps: [string, string][] = [["welkom", "Welkom"], ["gebed", "Gebed"], ["preek", "Preek"], ["slotzang", "Slotzang"]];
  return (
    <Card>
      <CardHeader><Zap /> Snel naar</CardHeader>
      <CardContent className="grid grid-cols-2 gap-2.5 sm:grid-cols-4">
        {jumps.map(([k, l]) => (
          <Button key={k} variant="outline" size="lg" onClick={() => go({ kind: k })}>{l}</Button>
        ))}
        <Button
          variant="outline"
          size="lg"
          className={cn("col-span-2 sm:col-span-4", state?.black && "border-primary bg-primary text-primary-foreground hover:bg-primary-hover")}
          onClick={() => go({ kind: "zwart" })}
        >
          {state?.black ? <><Eye /> Borden weer aan</> : <><EyeOff /> Borden zwart</>}
        </Button>
      </CardContent>
    </Card>
  );
}

export function Voorbereiden({ state, svc, status }: { state: View | null; svc?: Service; status: StreamStatus }) {
  const [settings, setSettings] = useState(false);
  return (
    <main className="mx-auto grid w-full max-w-[1500px] flex-1 content-start gap-5 p-5">
      <div className="flex flex-wrap items-end justify-between gap-3 px-1">
        <div>
          <h1 className="font-serif text-3xl font-semibold tracking-tight">Voorbereiden</h1>
          <p className="text-sm text-muted-foreground">Volgorde, teksten, preek en uitzending. Tijdens de dienst ga je terug naar “Dienst”.</p>
        </div>
        <Button variant="outline" size="lg" onClick={() => setSettings(true)}><Settings2 /> Instellingen</Button>
      </div>

      <div className="grid items-start gap-5 lg:grid-cols-[minmax(0,1.5fr)_minmax(340px,1fr)]">
        {svc && state ? (
          <OrderEditor svc={svc} state={state} />
        ) : (
          <Card className="grid place-items-center p-12 text-center text-muted-foreground">
            <div className="grid gap-3">
              <div className="text-lg font-semibold text-foreground">Nog geen dienst</div>
              <div>Haal de diensten op van de website, of maak er zelf een (bovenin).</div>
            </div>
          </Card>
        )}
        <div className="grid gap-5">
          {svc && <SermonCard key={svc.id} svc={svc} />}
          <FreeTextCard state={state} />
          <QuickJumps state={state} />
          <StreamCard status={status} onSettings={() => setSettings(true)} />
        </div>
      </div>

      {state?.slides?.length ? (
        <Card className="min-w-0">
          <CardHeader action={<span className="text-xs text-muted-foreground">tik om te tonen</span>}><Images /> Alle schermen</CardHeader>
          <CardContent className="pb-2"><Strip state={state} svc={svc} /></CardContent>
        </Card>
      ) : null}

      <SettingsDialog open={settings} onOpenChange={setSettings} />
    </main>
  );
}
