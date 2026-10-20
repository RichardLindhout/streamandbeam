import { useEffect, useMemo, useRef, useState } from "react";
import { go } from "@/lib/api";
import { useConsole } from "@/lib/state";
import { problems as findProblems } from "@/lib/problems";
import { chime } from "@/lib/chime";
import { Header, type Screen } from "@/components/Header";
import { AlarmBlock, Notices } from "@/components/Alarms";
import { Toaster } from "@/components/ui/sonner";
import { NewServiceDialog, SettingsDialog } from "@/components/Dialogs";
import { Dienst } from "@/views/Dienst";
import { Voorbereiden } from "@/views/Voorbereiden";

function stored(key: string) {
  try { return localStorage.getItem(key); } catch { return null; }
}
function store(key: string, v: string) {
  try { localStorage.setItem(key, v); } catch { /* private window */ }
}

function initialScreen(): Screen {
  const q = new URLSearchParams(location.search).get("scherm");
  if (q === "voorbereiden" || q === "dienst") return q;
  return stored("scherm") === "voorbereiden" ? "voorbereiden" : "dienst";
}

export default function App() {
  const { state, status, lis, yt, connected } = useConsole();
  const [screen, setScreen] = useState<Screen>(initialScreen);
  const [newOpen, setNewOpen] = useState(false);
  const [settings, setSettings] = useState(false);
  const [muted, setMuted] = useState(() => stored("stil") === "1");

  useEffect(() => store("scherm", screen), [screen]);
  useEffect(() => store("stil", muted ? "1" : "0"), [muted]);
  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if ((e.target as HTMLElement).closest("input, textarea, select, [role=dialog], [role=alertdialog], [role=menu]")) return;
      if (e.key === "ArrowRight" || e.key === "PageDown") { e.preventDefault(); go({ delta: 1 }); }
      if (e.key === "ArrowLeft" || e.key === "PageUp") { e.preventDefault(); go({ delta: -1 }); }
    };
    document.addEventListener("keydown", key);
    return () => document.removeEventListener("keydown", key);
  }, []);

  const problems = useMemo(() => findProblems(state, status, yt, connected), [state, status, yt, connected]);
  // A soft chime once when a serious problem starts.
  const alarm = problems.find((p) => p.severity === "rood")?.id;
  const last = useRef<string | undefined>(undefined);
  useEffect(() => {
    if (alarm && alarm !== last.current && !muted) chime();
    last.current = alarm;
  }, [alarm, muted]);

  const svc = state?.services?.find((s) => s.id === state.current);
  return (
    <div className="flex min-h-full flex-col">
      <Header
        state={state} svc={svc} status={status} lis={lis} yt={yt} problems={problems}
        screen={screen} setScreen={setScreen} onNew={() => setNewOpen(true)} onSettings={() => setSettings(true)}
      />
      <AlarmBlock problems={problems} muted={muted} setMuted={setMuted} />
      <Notices state={state} />
      {screen === "dienst" ? (
        <Dienst state={state} svc={svc} status={status} lis={lis} yt={yt} />
      ) : (
        <Voorbereiden state={state} svc={svc} onBack={() => setScreen("dienst")} />
      )}
      <NewServiceDialog open={newOpen} onOpenChange={setNewOpen} onMade={() => setScreen("voorbereiden")} />
      <SettingsDialog open={settings} onOpenChange={setSettings} state={state} status={status} />
      {/* No pop-up messages during the service; only while preparing. */}
      {screen === "voorbereiden" && <Toaster />}
    </div>
  );
}
