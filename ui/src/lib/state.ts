import { useEffect, useState } from "react";
import type { ListenStatus, StreamStatus, View, YouTubeCheck } from "./api";

/** Everything the console shows: the show (via SSE from slide.js) and the polled statuses. */
export function useConsole() {
  const [state, setState] = useState<View | null>(null);
  const [status, setStatus] = useState<StreamStatus>({ mode: "stop", error: "", audioDb: -100, recording: "", audioWarning: "", since: 0, boards: 0 });
  const [lis, setLis] = useState<ListenStatus | null>(null);
  const [yt, setYt] = useState<YouTubeCheck | null>(null);
  const [connected, setConnected] = useState(true);
  const [, tick] = useState(0);

  useEffect(() => listen(setState), []);
  useEffect(() => {
    const poll = <T,>(url: string, set: (v: T) => void, ok?: (b: boolean) => void) => async () => {
      try {
        set(await (await fetch(url)).json());
        ok?.(true);
      } catch {
        ok?.(false);
      }
    };
    const s = poll("/api/stream", setStatus, setConnected);
    const l = poll("/api/listen", setLis);
    const y = poll("/api/youtube-check", setYt);
    s(); l(); y();
    const t = [setInterval(s, 250), setInterval(l, 1500), setInterval(y, 10000), setInterval(() => tick((n) => n + 1), 1000)];
    return () => t.forEach(clearInterval);
  }, []);

  return { state, status, lis, yt, connected };
}
