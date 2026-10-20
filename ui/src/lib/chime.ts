// A short, soft two-note chime for when an alarm starts (never loops).
let ctx: AudioContext | null = null;

function context() {
  if (!ctx) {
    const C = window.AudioContext || (window as unknown as { webkitAudioContext: typeof AudioContext }).webkitAudioContext;
    if (!C) return null;
    ctx = new C();
  }
  return ctx;
}

// Browsers only allow sound after the first tap on the page.
if (typeof window !== "undefined") {
  window.addEventListener("pointerdown", () => context()?.resume(), { once: true });
}

export function chime() {
  const c = context();
  if (!c) return;
  c.resume();
  [659.25, 523.25].forEach((f, i) => {
    const o = c.createOscillator();
    const g = c.createGain();
    o.type = "sine";
    o.frequency.value = f;
    const t = c.currentTime + i * 0.28;
    g.gain.setValueAtTime(0, t);
    g.gain.linearRampToValueAtTime(0.18, t + 0.03);
    g.gain.exponentialRampToValueAtTime(0.001, t + 0.9);
    o.connect(g).connect(c.destination);
    o.start(t);
    o.stop(t + 1);
  });
}
