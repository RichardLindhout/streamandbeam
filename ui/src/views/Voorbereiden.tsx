import { ArrowLeft } from "lucide-react";
import type { Service, View } from "@/lib/api";
import { Button } from "@/components/ui/button";
import { Card } from "@/components/ui/card";
import { OrderEditor } from "@/components/OrderEditor";
import { FreeTextCard, SermonCard } from "@/components/PrepareCards";

export function Voorbereiden({ state, svc, onBack }: { state: View | null; svc?: Service; onBack: () => void }) {
  return (
    <main className="mx-auto grid w-full max-w-[1500px] flex-1 content-start gap-5 p-5">
      <div className="flex flex-wrap items-center gap-4 px-1">
        <Button size="lg" onClick={onBack}><ArrowLeft /> Terug naar de dienst</Button>
        <h1 className="font-serif text-3xl font-semibold tracking-tight">Dienst voorbereiden</h1>
      </div>

      <div className="grid items-start gap-5 lg:grid-cols-[minmax(0,1.5fr)_minmax(340px,1fr)]">
        {svc && state ? (
          <OrderEditor svc={svc} state={state} />
        ) : (
          <Card className="grid place-items-center p-12 text-center text-muted-foreground">
            <div className="grid gap-3">
              <div className="font-serif text-xl font-semibold text-foreground">Nog geen dienst</div>
              <div>Kies in het menu “Diensten ophalen van website” of “Nieuwe dienst”.</div>
            </div>
          </Card>
        )}
        <div className="grid gap-5">
          {svc && <SermonCard key={svc.id} svc={svc} />}
          <FreeTextCard state={state} />
        </div>
      </div>
    </main>
  );
}
