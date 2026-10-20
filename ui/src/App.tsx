import { useEffect, useState } from "react";
import { go } from "@/lib/api";
import { useConsole } from "@/lib/state";
import { Header, type Screen } from "@/components/Header";
import { Alarms } from "@/components/Alarms";
import { Toaster } from "@/components/ui/sonner";
import { BigMonitor, Dienst } from "@/views/Dienst";
import { Voorbereiden } from "@/views/Voorbereiden";
import { NewServiceDialog } from "@/components/Dialogs";

function initialScreen(): Screen {
  const q = new URLSearchParams(location.search).get("scherm");
  if (q === "voorbereiden" || q === "dienst") return q;
  try {
    return localStorage.getItem("scherm") === "voorbereiden" ? "voorbereiden" : "dienst";
  } catch {
    return "dienst";
  }
}

export default function App() {
  const { state, status, lis, yt, connected } = useConsole();
  const [screen, setScreen] = useState<Screen>(initialScreen);
  const [big, setBig] = useState<"bord" | "stream" | null>(null);
  const [newOpen, setNewOpen] = useState(false);

  useEffect(() => {
    try { localStorage.setItem("scherm", screen); } catch { /* private window */ }
  }, [screen]);
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if ((e.target as HTMLElement).closest("input, textarea, select, [role=dialog], [role=alertdialog]")) return;
      if (e.key === "ArrowRight" || e.key === "PageDown") { e.preventDefault(); go({ delta: 1 }); }
      if (e.key === "ArrowLeft" || e.key === "PageUp") { e.preventDefault(); go({ delta: -1 }); }
    };
    document.addEventListener("keydown", key);
    return () => document.removeEventListener("keydown", key);
  }, []);

  const svc = state?.services?.find((s) => s.id === state.current);
  return (
    <div className="flex min-h-full flex-col">
      <Alarms state={state} status={status} yt={yt} connected={connected} />
      <Header state={state} onNew={() => setNewOpen(true)} svc={svc} status={status} lis={lis} yt={yt} screen={screen} setScreen={setScreen} />
      {screen === "dienst" ? (
        <Dienst state={state} svc={svc} status={status} lis={lis} onBig={setBig} />
      ) : (
        <Voorbereiden state={state} svc={svc} status={status} />
      )}
      {big && <BigMonitor which={big} state={state} status={status} close={() => setBig(null)} />}
      <NewServiceDialog open={newOpen} onOpenChange={setNewOpen} onMade={() => setScreen("voorbereiden")} />
      <Toaster />
    </div>
  );
}
