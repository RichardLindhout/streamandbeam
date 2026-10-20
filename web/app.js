// Stream & Beam — operator console (Preact + htm, no build step; works offline).
// Two modes: "Dienst" (operate only: nothing can be deleted by accident) and
// "Voorbereiden" (type, drag, edit). Live monitors of the boards and of what YouTube gets,
// and a strip of clickable slides.
import { html, render, useState, useEffect, useRef, useMemo } from "/vendor/preact-htm.js";

const post = (url, body) => fetch(url, { method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(body ?? {}) });
const go = (body) => post("/api/go", body);
const ICON = { welkom: "⌂", psalm: "♪", slotzang: "♪", nazang: "♪", lied: "♫", lezing: "✚", catechismus: "HC", belijdenis: "§", formulier: "¶", gebed: "✋", collecte: "€", preek: "✎", mededeling: "i", einde: "■" };

function hash(s) {
  let h = 0;
  for (let i = 0; i < s.length; i++) h = (h * 31 + s.charCodeAt(i)) | 0;
  return (h >>> 0).toString(36);
}

function clock(since) {
  if (!since) return "";
  const s = Math.max(0, Math.floor(Date.now() / 1000 - since));
  const p = (n) => String(n).padStart(2, "0");
  return `${p(Math.floor(s / 3600))}:${p(Math.floor((s % 3600) / 60))}:${p(s % 60)}`;
}

// One board, drawn with the same code as the TVs.
function Board({ slide, black, className, onClick }) {
  const ref = useRef();
  useEffect(() => {
    if (ref.current) renderSlide(ref.current, slide, black, { portrait: true });
  });
  useEffect(() => {
    const redraw = () => ref.current && renderSlide(ref.current, slide, black, { portrait: true });
    window.addEventListener("resize", redraw);
    return () => window.removeEventListener("resize", redraw);
  }, [slide, black]);
  return html`<div class=${"mon portrait " + (className || "")} onClick=${onClick}><span class="tag">Bord in de kerk</span><div class="canvas" ref=${ref}></div></div>`;
}

function StreamMonitor({ mode, onClick }) {
  const on = mode && mode !== "stop";
  return html`<div class="mon land" onClick=${onClick}>
    <span class=${"tag" + (mode === "live" ? " live" : "")}>${mode === "live" ? "● Live op YouTube" : "Uitzending"}</span>
    ${on ? html`<img src="/preview.mjpg" alt="" />` : html`<div class="off">De uitzending staat uit.<br />Kies rechts “Voorbeeld” om het beeld te zien.</div>`}
  </div>`;
}

function Header({ state, status, mode, setMode, onNew, onFetch, fetching, listen }) {
  const svcs = state?.services || [];
  const live = status.mode === "live";
  return html`<header class="top">
    <div class="brand"><i>✝</i><div>Stream & Beam<small>HHG Tholen</small></div></div>
    <div class="svc">
      <select value=${state?.current} onChange=${(e) => post("/api/select?id=" + encodeURIComponent(e.target.value))}>
        ${svcs.length ? svcs.map((s) => html`<option value=${s.id}>${s.date} · ${s.time} · ${s.label}${s.predikant ? " · " + s.predikant : ""}</option>`) : html`<option>Nog geen diensten</option>`}
      </select>
      <button class="btn" onClick=${onFetch} disabled=${fetching} title="Diensten ophalen van hhgtholen.nl">${fetching ? "Bezig…" : "↻ Website"}</button>
      <button class="btn" onClick=${onNew}>+ Dienst</button>
    </div>
    <div class="seg">
      <button class=${mode === "dienst" ? "on" : ""} onClick=${() => setMode("dienst")}>Dienst</button>
      <button class=${mode === "voorbereiden" ? "on" : ""} onClick=${() => setMode("voorbereiden")}>Voorbereiden</button>
    </div>
    <div class="pills">
      <span class="pill"><span class=${"dot " + (status.boards > 0 ? "ok" : "warn")}></span>Borden <b>${status.boards || 0}</b></span>
      <span class=${"pill" + (live ? " live" : "")}><span class=${"dot " + (live ? "live" : status.mode && status.mode !== "stop" ? "ok" : "")}></span>
        ${live ? html`<b>LIVE</b> ${clock(status.since)}` : status.mode === "opnemen" ? html`<b>Opname</b> ${clock(status.since)}` : status.mode === "voorbeeld" ? "Voorbeeld" : "Niet live"}</span>
      <span class="pill"><span class=${"dot " + (status.mode === "stop" ? "" : status.audioWarning ? "warn" : "ok")}></span>Geluid</span>
      ${listen?.active ? html`<span class="pill"><span class="dot ok"></span>Meeluisteren</span>` : null}
    </div>
  </header>`;
}

function Order({ svc, state, prepare, save }) {
  const [typed, setTyped] = useState("");
  const [busy, setBusy] = useState("");
  const [after, setAfter] = useState(null);
  const [edit, setEdit] = useState(null); // { i, slides }
  const [drag, setDrag] = useState(null);
  const [undo, setUndo] = useState(null);
  const cur = state?.slides?.[state.index]?.item;
  const nxt = state?.slides?.slice(state.index + 1).find((s) => s.item !== cur)?.item;
  if (!svc) return html`<div class="panel"><h3>Volgorde</h3><div class="body hint">Kies of maak een dienst.</div></div>`;

  const update = (items) => save({ ...svc, items });
  async function add() {
    if (!typed.trim()) return;
    setBusy("Bezig met herkennen…");
    const r = await post("/api/quick", { text: typed });
    if (!r.ok) return setBusy("Fout: " + (await r.text()));
    const items = (await r.json()).map((it, k) => ({ ...it, id: "i" + Date.now() + k }));
    const list = [...svc.items];
    let at = after !== null ? after + 1 : list.findIndex((i) => i.kind === "einde");
    if (at < 0) at = list.length;
    list.splice(at, 0, ...items);
    setAfter(at + items.length - 1);
    update(list);
    const unsure = items.filter((i) => i.problem).length;
    setBusy(`${items.length} toegevoegd` + (unsure ? ` — ${unsure} niet zeker, zie ⚠` : ""));
    setTyped("");
  }
  function move(i, d) {
    const j = i + d;
    if (j < 0 || j >= svc.items.length) return;
    const list = [...svc.items];
    [list[i], list[j]] = [list[j], list[i]];
    update(list);
  }
  function remove(i) {
    const list = [...svc.items];
    const [gone] = list.splice(i, 1);
    update(list);
    setUndo({ item: gone, i });
    setTimeout(() => setUndo((u) => (u && u.item === gone ? null : u)), 8000);
  }
  function openEdit(i) {
    const it = svc.items[i];
    const slides = it.custom?.length
      ? it.custom.map((c) => ({ ...c }))
      : (state.slides || []).filter((s) => s.item === i).map((s) => ({
          header: s.header, sub: s.sub, footer: s.footer || "",
          text: [...(s.lines || []), ...(s.verses || []).map((v) => v.verse + " " + v.text), ...(s.theme ? [s.theme] : []), ...(s.points || []).map((p, k) => `${k + 1}. ${p}`), ...(s.list || [])].join("\n"),
        }));
    setEdit({ i, slides });
  }
  function click(i) {
    setAfter(i);
    const slide = (state.slides || []).findIndex((s) => s.item === i);
    if (slide >= 0) go({ index: slide });
  }

  return html`<div class="panel">
    <h3><span class="grow">Volgorde</span>${prepare ? html`<span class="hint" style="text-transform:none;letter-spacing:0">sleep om te verplaatsen</span>` : null}</h3>
    ${prepare && html`<div class="typer">
      <input class="input" value=${typed} onInput=${(e) => setTyped(e.target.value)} onKeyDown=${(e) => e.key === "Enter" && add()}
        placeholder="ps 42:5, gebed, joh 3:16, HC 57, NGB art. 14, levensloop" />
      <button class="btn primary" onClick=${add}>Voeg toe</button>
    </div>`}
    ${prepare && busy && html`<div class="hint" style="padding:6px 12px">${busy}</div>`}
    ${prepare && (svc.problems || []).length > 0 && html`<div class="note" style="margin:8px 10px">Niet herkend op de website: ${svc.problems.join("; ")}</div>`}
    <ul class="order">
      ${svc.items.map((it, i) => html`<li key=${it.id || i} class=${(i === cur ? "now " : i === nxt ? "next " : "") + (drag === i ? "drag" : "")}
          draggable=${prepare} onDragStart=${() => setDrag(i)} onDragOver=${(e) => e.preventDefault()}
          onDrop=${() => { if (drag === null) return; const list = [...svc.items]; const [m] = list.splice(drag, 1); list.splice(i, 0, m); setDrag(null); update(list); }}
          onClick=${(e) => !e.target.closest(".tools, .editor") && click(i)}>
        <span class="ico">${ICON[it.kind] || "•"}</span>
        <span class="what"><b>${it.label}</b><span>${it.ref}</span>${it.custom?.length ? html`<span>✎</span>` : null}
          ${it.problem ? html`<span class="warn">⚠ ${it.problem}</span>` : null}</span>
        ${prepare ? html`<span class="tools">
          <button title="Tekst aanpassen" onClick=${() => openEdit(i)}>✎</button>
          <button title="Omhoog" onClick=${() => move(i, -1)}>↑</button>
          <button title="Omlaag" onClick=${() => move(i, 1)}>↓</button>
          <button title="Weghalen" onClick=${() => remove(i)}>✕</button></span>` : html`<span></span>`}
        ${prepare && (it.kind === "collecte" || it.kind === "mededeling") && html`<textarea class="input editor" rows="2" placeholder=${it.kind === "collecte" ? "Collectedoelen, één per regel" : "Tekst (mag leeg)"}
          value=${it.text} onChange=${(e) => update(svc.items.map((x, k) => (k === i ? { ...x, text: e.target.value } : x)))}></textarea>`}
        ${edit && edit.i === i && html`<div class="editor">
          ${edit.slides.map((c, k) => html`<div style="display:grid;gap:5px">
            <input class="input" value=${c.header} placeholder="Kop" onInput=${(e) => { c.header = e.target.value; }} />
            <textarea class="input" rows="4" onInput=${(e) => { c.text = e.target.value; }}>${c.text}</textarea>
          </div>`)}
          <div style="display:flex;gap:6px;flex-wrap:wrap">
            <button class="btn primary" onClick=${() => { update(svc.items.map((x, k) => (k === i ? { ...x, custom: edit.slides } : x))); setEdit(null); }}>Opslaan</button>
            <button class="btn" onClick=${() => setEdit({ ...edit, slides: [...edit.slides, { header: it.label, sub: "", text: "", footer: "" }] })}>+ Dia</button>
            <button class="btn" onClick=${() => { update(svc.items.map((x, k) => (k === i ? { ...x, custom: [] } : x))); setEdit(null); }}>Herstel origineel</button>
            <button class="btn ghost" onClick=${() => setEdit(null)}>Annuleer</button>
          </div></div>`}
      </li>`)}
    </ul>
    ${prepare && html`<${Sermon} svc=${svc} save=${save} />`}
    ${undo && html`<div class="toast">“${undo.item.label}” weggehaald
      <button class="btn" onClick=${() => { const list = [...svc.items]; list.splice(undo.i, 0, undo.item); update(list); setUndo(null); }}>Ongedaan maken</button></div>`}
  </div>`;
}

function Sermon({ svc, save }) {
  const [text, setText] = useState(svc.text?.ref || "");
  useEffect(() => setText(svc.text?.ref || ""), [svc.id]);
  return html`<div class="body" style="border-top:1px solid var(--line);display:grid;gap:8px">
    <div class="hint">Preek</div>
    <label class="field">Predikant<input value=${svc.predikant} onChange=${(e) => save({ ...svc, predikant: e.target.value })} /></label>
    <label class="field">Tekst (bijv. joh 5:25 of HC 57)<input value=${text} onInput=${(e) => setText(e.target.value)}
      onChange=${async () => { const r = await post("/api/quick", { text }); if (r.ok) { const [it] = await r.json(); if (it) save({ ...svc, text: it }); } }} /></label>
    <label class="field">Thema<input value=${svc.theme} onChange=${(e) => save({ ...svc, theme: e.target.value })} /></label>
    <label class="field">Punten (één per regel)<textarea rows="3" onChange=${(e) => save({ ...svc, points: e.target.value.split("\n").map((s) => s.trim()).filter(Boolean) })}>${(svc.points || []).join("\n")}</textarea></label>
    <button class="btn danger" onClick=${() => confirm("Deze dienst verwijderen?") && post("/api/delete-service?id=" + encodeURIComponent(svc.id))}>Dienst verwijderen</button>
  </div>`;
}

function Strip({ state, svc }) {
  const ref = useRef();
  useEffect(() => {
    ref.current?.querySelector(".now")?.scrollIntoView({ block: "nearest", inline: "center", behavior: "smooth" });
  }, [state?.index]);
  if (!state?.slides?.length) return null;
  return html`<div class="panel" style="margin-top:14px">
    <h3><span class="grow">Alle schermen — klik om te tonen</span></h3>
    <div class="strip" ref=${ref}>
      ${state.slides.map((s, i) => html`<div class=${"card" + (i === state.index ? " now" : i === state.index + 1 ? " next" : "")} onClick=${() => go({ index: i })} key=${i}>
        <img loading="lazy" src=${`/api/thumb?index=${i}&k=${hash(state.current + JSON.stringify(s))}`} alt="" />
        <div>${s.header}<small>${s.sub || svc?.items?.[s.item]?.label || ""}</small></div>
      </div>`)}
    </div>
  </div>`;
}

function Settings({ close }) {
  const [s, setS] = useState(null);
  const [yt, setYt] = useState(null);
  useEffect(() => { fetch("/api/settings").then((r) => r.json()).then(setS); fetch("/api/youtube").then((r) => r.json()).then(setYt); }, []);
  if (!s) return null;
  const f = (k, label, type = "text") => html`<label class="field">${label}<input type=${type} value=${s[k]} onInput=${(e) => setS({ ...s, [k]: type === "number" ? +e.target.value : e.target.value })} /></label>`;
  return html`<div class="modal" onClick=${(e) => e.target.classList.contains("modal") && close()}><div class="box">
    <h3>Instellingen</h3>
    <div class="body">
      <div class="two">${f("cameraIp", "IP-adres camera (leeg = testbeeld)")}${f("audioSource", "Geluidsbron (leeg = standaard)")}</div>
      <label class="field">Camerapositie overzicht / orgel / preekstoel / doopvont
        <div class="four">${["overzicht", "orgel", "preekstoel", "doopvont"].map((p) => html`<input type="number" value=${s.presets[p]} onInput=${(e) => setS({ ...s, presets: { ...s.presets, [p]: +e.target.value } })} />`)}</div></label>
      <div class="two">${f("introSeconds", "Aankondiging psalm (sec)", "number")}${f("themeSeconds", "Thema in beeld (sec)", "number")}</div>
      <div class="two">${f("autoStartMinutes", "Automatisch live (min vooraf, 0 = uit)", "number")}${f("autoStopMinutes", "Automatisch stoppen (min na einde)", "number")}</div>
      <label class="field">Meeluisteren<select value=${s.autopilot} onChange=${(e) => setS({ ...s, autopilot: e.target.value })}>
        <option value="uit">Uit</option><option value="suggestie">Suggestie (knop kleurt)</option><option value="automatisch">Automatisch als het zeker is</option></select></label>
      <div class="hint">YouTube: ${yt?.channel ? html`ingelogd als <b>${yt.channel}</b>` : yt?.configured ? html`<a href="/youtube/login" style="color:var(--accent)">inloggen met Google</a> (op deze PC)` : "vul hieronder de Google-gegevens in, of gebruik een streamsleutel"}</div>
      <div class="two">${f("youtubeClientId", "Google client-ID")}${f("youtubeClientSecret", "Google client-geheim", "password")}</div>
      <div class="two">${f("streamKey", "Of: YouTube-streamsleutel", "password")}${f("qrLink", "Link in QR-code")}</div>
      <div class="two">${f("recordDir", "Map voor opnames")}${f("videoEncoder", "Video-encoder")}</div>
      ${f("cameraRtsp", "Camera RTSP-adres (leeg = standaard)")}
    </div>
    <div class="foot"><button class="btn ghost" onClick=${close}>Annuleer</button>
      <button class="btn primary" onClick=${async () => { await post("/api/settings", s); close(); }}>Opslaan</button></div>
  </div></div>`;
}

function NewService({ close }) {
  const sunday = new Date(); sunday.setDate(sunday.getDate() + ((7 - sunday.getDay()) % 7));
  const [v, setV] = useState({ date: sunday.toISOString().slice(0, 10), time: "09:30", label: "Morgendienst" });
  const kinds = ["Morgendienst", "Middagdienst", "Avonddienst", "Rouwdienst", "Huwelijksdienst", "Bevestigingsdienst", "Doopdienst", "Heilig Avondmaal", "Voorbereiding Heilig Avondmaal", "Dankzegging Heilig Avondmaal", "Bidstond", "Dankdag", "Kerstfeest", "Oudejaarsavond", "Goede Vrijdag", "Pasen", "Hemelvaart", "Pinksteren", "Kinderzangdienst"];
  return html`<div class="modal" onClick=${(e) => e.target.classList.contains("modal") && close()}><div class="box">
    <h3>Nieuwe dienst</h3>
    <div class="body">
      <div class="two"><label class="field">Datum<input type="date" value=${v.date} onInput=${(e) => setV({ ...v, date: e.target.value })} /></label>
        <label class="field">Tijd<input type="time" value=${v.time} onInput=${(e) => setV({ ...v, time: e.target.value })} /></label></div>
      <label class="field">Soort dienst (vrij in te vullen)<input list="kinds" value=${v.label} onInput=${(e) => setV({ ...v, label: e.target.value })} />
        <datalist id="kinds">${kinds.map((k) => html`<option>${k}</option>`)}</datalist></label>
      <div class="hint">Daarna typ je in “Voorbereiden” wat er komt, in elke volgorde — ook bij een rouw- of huwelijksdienst.</div>
    </div>
    <div class="foot"><button class="btn ghost" onClick=${close}>Annuleer</button>
      <button class="btn primary" onClick=${async () => { await post("/api/new-service", v); close(true); }}>Maak</button></div>
  </div></div>`;
}

function Big({ which, state, status, close }) {
  const slide = state && (state.overrideBoard || (state.boards || state.slides)?.[state.index]);
  return html`<div class="modal" onClick=${close}>
    <div style="width:min(1400px,96vw)">${which === "bord"
      ? html`<div style="height:86vh;aspect-ratio:9/16;margin:0 auto"><${Board} slide=${slide} black=${state?.black} className="" /></div>`
      : html`<${StreamMonitor} mode=${status.mode} />`}</div></div>`;
}

function App() {
  const [state, setState] = useState(null);
  const [status, setStatus] = useState({ mode: "stop" });
  const [lis, setLis] = useState(null);
  const [mode, setMode] = useState(() => localStorage.getItem("modus") || "dienst");
  const [modal, setModal] = useState(null);
  const [fetching, setFetching] = useState(false);
  const [freeText, setFreeText] = useState({ title: "", text: "", stream: false });
  const [connected, setConnected] = useState(true);
  const editing = useRef(false);

  useEffect(() => listen((st) => setState(st)), []);
  useEffect(() => {
    const t1 = setInterval(async () => {
      try { setStatus(await (await fetch("/api/stream")).json()); setConnected(true); } catch { setConnected(false); }
    }, 700);
    const t2 = setInterval(async () => { try { setLis(await (await fetch("/api/listen")).json()); } catch {} }, 1500);
    return () => { clearInterval(t1); clearInterval(t2); };
  }, []);
  useEffect(() => localStorage.setItem("modus", mode), [mode]);
  useEffect(() => {
    const key = (e) => {
      if (e.target.closest("input, textarea, select")) return;
      if (e.key === "ArrowRight" || e.key === "PageDown") { e.preventDefault(); go({ delta: 1 }); }
      if (e.key === "ArrowLeft" || e.key === "PageUp") { e.preventDefault(); go({ delta: -1 }); }
    };
    document.addEventListener("keydown", key);
    return () => document.removeEventListener("keydown", key);
  }, []);

  const svc = state?.services?.find((s) => s.id === state.current);
  const boards = state?.boards || state?.slides || [];
  const boardSlide = state?.overrideBoard || boards[state?.index ?? 0];
  const nextSlide = state?.slides?.[(state?.index ?? 0) + 1];
  const suggestion = lis?.suggestion;
  const save = (s) => post("/api/service", s);
  const setStreamMode = async (m) => {
    if (m === "live" && !confirm("Live gaan op YouTube?")) return;
    if (m === "stop" && status.mode === "live" && !confirm("Uitzending stoppen?")) return;
    const r = await post("/api/stream", { mode: m });
    if (!r.ok) alert("Lukt niet: " + (await r.text()));
  };
  const pct = Math.max(0, Math.min(100, ((status.audioDb ?? -100) + 60) / 60 * 100));

  return html`
    ${!connected && html`<div class="alarm grey">Geen verbinding met de kerk-PC — controleer of het programma draait en het netwerk werkt</div>`}
    ${status.audioWarning && html`<div class="alarm">⚠ ${status.audioWarning}</div>`}
    ${state?.overrideBoard && html`<div class="alarm grey" style="background:#7a5a12">Het bord toont nu je vrije tekst
      <button class="btn" style="margin-left:12px" onClick=${() => post("/api/board-override", { clear: true })}>Terug naar normaal</button></div>`}
    ${state?.black && html`<div class="alarm grey">De borden zijn ZWART <button class="btn" style="margin-left:12px" onClick=${() => go({ kind: "zwart" })}>Weer aan</button></div>`}
    <${Header} state=${state} status=${status} mode=${mode} setMode=${setMode} listen=${lis}
      onNew=${() => setModal("nieuw")} fetching=${fetching}
      onFetch=${async () => { setFetching(true); const r = await post("/api/fetch"); setFetching(false); if (!r.ok) alert("Ophalen mislukt: " + (await r.text())); }} />
    <main class="grid">
      <${Order} svc=${svc} state=${state} prepare=${mode === "voorbereiden"} save=${save} />
      <section>
        <div class="monitors">
          <${Board} slide=${boardSlide} black=${state?.black} onClick=${() => setModal("bord")} />
          <div>
            <${StreamMonitor} mode=${status.mode} onClick=${() => setModal("stream")} />
            <div class="meter"><div style=${{ width: (status.mode === "stop" ? 0 : pct) + "%", background: status.audioDb > -3 ? "var(--live)" : status.audioDb > -12 ? "var(--warn)" : "var(--ok)" }}></div></div>
            <div class="go">
              <button onClick=${() => go({ delta: -1 })}>◀ Vorige</button>
              <button class=${"next" + (suggestion ? " suggest" : "")} onClick=${() => (suggestion ? post("/api/listen/accept") : go({ delta: 1 }))}>
                <span>${suggestion ? "Volgens het gehoorde:" : "Volgende"}
                  <small>${suggestion ? `${suggestion.label} — “${suggestion.reason}”` : nextSlide ? `${nextSlide.header}${nextSlide.sub ? " · " + nextSlide.sub : ""}` : "einde van de dienst"}</small></span>
                <span class="arrow">▶</span>
              </button>
            </div>
            ${lis?.heard && html`<div class="heard" style="margin-top:8px">Gehoord: ${lis.heard}</div>`}
          </div>
        </div>
        <${Strip} state=${state} svc=${svc} />
      </section>
      <aside class="right stack">
        <div class="panel"><h3>Direct naar</h3><div class="body jumps">
          <button class="btn" onClick=${() => go({ kind: "welkom" })}>Welkom</button>
          <button class="btn" onClick=${() => go({ kind: "gebed" })}>Gebed</button>
          <button class="btn" onClick=${() => go({ kind: "preek" })}>Preek</button>
          <button class="btn" onClick=${() => go({ kind: "slotzang" })}>Slotzang</button>
          <button class=${"btn wide" + (state?.black ? " on" : "")} onClick=${() => go({ kind: "zwart" })}>${state?.black ? "Borden weer aan" : "Borden zwart"}</button>
        </div></div>
        <div class="panel"><h3>Vrije tekst op het bord</h3><div class="body" style="display:grid;gap:8px">
          <input class="input" placeholder="Bovenste regel (groot)" value=${freeText.title} onInput=${(e) => setFreeText({ ...freeText, title: e.target.value })} />
          <textarea class="input" rows="4" placeholder="Eén regel per richel" value=${freeText.text} onInput=${(e) => setFreeText({ ...freeText, text: e.target.value })}></textarea>
          ${(freeText.title || freeText.text) && html`<div style="width:140px;margin:0 auto"><${Board} slide=${{ kind: "vrij", header: freeText.title, lines: freeText.text.split("\n") }} /></div>`}
          <label class="hint" style="display:flex;gap:6px;align-items:center"><input type="checkbox" checked=${freeText.stream} onChange=${(e) => setFreeText({ ...freeText, stream: e.target.checked })} /> ook in de uitzending</label>
          <button class="btn primary" onClick=${() => post("/api/board-override", freeText)}>Plaatsen</button>
        </div></div>
        <div class="panel"><h3><span class="grow">Uitzending</span><button class="btn ghost" style="padding:2px 8px" onClick=${() => setModal("instellingen")}>⚙</button></h3>
          <div class="body" style="display:grid;gap:8px">
            <div class="two">
              <button class=${"btn" + (status.mode === "voorbeeld" ? " on" : "")} onClick=${() => setStreamMode("voorbeeld")}>Voorbeeld</button>
              <button class=${"btn" + (status.mode === "opnemen" ? " on" : "")} onClick=${() => setStreamMode("opnemen")}>Opnemen</button>
              <button class=${"btn live" + (status.mode === "live" ? " on" : "")} onClick=${() => setStreamMode("live")}>● Live</button>
              <button class="btn" onClick=${() => setStreamMode("stop")}>Stop</button>
            </div>
            ${status.recording && html`<div class="hint">Opname: ${status.recording.split("/").pop()}</div>`}
            ${status.error && html`<div class="note">${status.error}</div>`}
          </div></div>
      </aside>
    </main>
    ${modal === "instellingen" && html`<${Settings} close=${() => setModal(null)} />`}
    ${modal === "nieuw" && html`<${NewService} close=${(made) => { setModal(null); if (made) setMode("voorbereiden"); }} />`}
    ${(modal === "bord" || modal === "stream") && html`<${Big} which=${modal} state=${state} status=${status} close=${() => setModal(null)} />`}
  `;
}

render(html`<${App} />`, document.getElementById("app"));
