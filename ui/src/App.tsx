import { useEffect, useMemo, useRef, useState } from "react";
import { setStreamMode, step } from "@/lib/api";
import { useConsole } from "@/lib/state";
import { alarms as findAlarms, type Alarm } from "@/lib/problems";
import { chime } from "@/lib/chime";
import { Header, type Screen } from "@/components/Header";
import { AlarmBand, Notice } from "@/components/Alarms";
import { Toaster } from "@/components/ui/sonner";
import { SettingsDialog } from "@/components/Dialogs";
import { Dienst } from "@/views/Dienst";
import { Voorbereiden } from "@/views/Voorbereiden";

function initialScreen(): Screen {
  const q = new URLSearchParams(location.search).get("scherm");
  return q === "voorbereiden" ? "voorbereiden" : "dienst";
}

/** True while a dialog, sheet or menu is open, or the cursor is in a text field. */
function busy(e: KeyboardEvent) {
  if ((e.target as HTMLElement | null)?.closest?.("input, textarea, select, [contenteditable=true]")) return true;
  return !!document.querySelector("[role=dialog], [role=alertdialog], [role=menu]");
}

export default function App() {
  const { state, status, lis, yt, connected } = useConsole();
  const [screen, setScreen] = useState<Screen>(initialScreen);
  const [settings, setSettings] = useState(false);
  const [seen, setSeen] = useState<string[]>([]);

  useEffect(() => {
    const key = (e: KeyboardEvent) => {
      if (e.key !== "PageDown" && e.key !== "PageUp") return;
      if (busy(e)) return;
      e.preventDefault();
      step(e.key === "PageDown" ? 1 : -1);
    };
    document.addEventListener("keydown", key);
    return () => document.removeEventListener("keydown", key);
  }, []);

  const svc = state?.services?.find((s) => s.id === state.current);
  const now = Math.floor(Date.now() / 30_000); // re-check time-based alarms every half minute
  const active = useMemo(() => findAlarms(state, svc, status, yt), [state, svc, status, yt, now]); // eslint-disable-line react-hooks/exhaustive-deps
  const ids = active.map((a) => a.id).join(",");
  // "Gezien" lasts until the condition clears; when it comes back, the alarm shows again.
  useEffect(() => setSeen((s) => (s.some((id) => !active.some((a) => a.id === id)) ? s.filter((id) => active.some((a) => a.id === id)) : s)), [ids]); // eslint-disable-line react-hooks/exhaustive-deps
  const alarm = active.find((a) => !seen.includes(a.id));

  const last = useRef<string | undefined>(undefined);
  useEffect(() => {
    if (alarm && alarm.id !== last.current) chime();
    last.current = alarm?.id;
  }, [alarm?.id]); // eslint-disable-line react-hooks/exhaustive-deps

  function act(a: Alarm) {
    if (a.action === "retry") setStreamMode("live");
    else if (a.action === "begin") step(1);
    else setSeen((s) => [...s, a.id]);
  }

  return (
    <div className="flex h-full flex-col overflow-hidden">
      {screen === "dienst" && (
        <Header
          state={state} svc={svc} status={status} yt={yt} connected={connected} alarm={!!active.length}
          setScreen={setScreen} onSettings={() => setSettings(true)}
        />
      )}
      {alarm ? <AlarmBand alarm={alarm} onAct={act} /> : screen === "dienst" ? <Notice state={state} /> : null}
      {screen === "dienst" ? (
        <Dienst state={state} svc={svc} status={status} lis={lis} />
      ) : (
        <Voorbereiden state={state} status={status} onBack={() => setScreen("dienst")} />
      )}
      <SettingsDialog
        open={settings} onOpenChange={setSettings} state={state} status={status}
        onMade={() => { setSettings(false); setScreen("voorbereiden"); }}
      />
      <Toaster />
    </div>
  );
}
