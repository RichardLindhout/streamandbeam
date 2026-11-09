// Shared by the screens (beam.html) and the preview in the operator page.
// Keep this to syntax that older smart-TV browsers understand (no ?. or ??).

function esc(s) {
  return String(s == null ? "" : s).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);
}

// Text for the screens: escaped, with a typographic apostrophe (’t instead of 't).
function typo(s) {
  return esc(String(s == null ? "" : s).replace(/'/g, "\u2019"));
}

function slideHTML(s) {
  if (!s) return "";
  const bar = `<div class="bar"><h1>${typo(s.header)}</h1><span>${typo(s.sub)}</span></div>`;
  switch (s.kind) {
    case "bord":
      return `<div class="rows">${(s.list || []).map((l) => `<div class="row">${typo(l)}</div>`).join("")}</div>`;
    // Free text from the operator, set on the ledges like the psalm board.
    case "vrij":
      return `<div class="rows">${[s.header].concat(s.lines || []).filter((l, i) => i > 0 || l).map((l, i) =>
        `<div class="row${i === 0 && s.header ? " head" : ""}">${typo(l)}</div>`).join("")}</div>`;
    // The theme during the sermon: every line of text sits on a ledge.
    case "thema":
      return `<div class="flow">${s.sub ? `<div class="ref">Tekst: ${typo(s.sub)}</div>` : ""}<div class="t">${typo(s.theme)}</div>${(s.points || []).map((p, i) => `<div>${i + 1}. ${typo(p)}</div>`).join("")}</div>`;
    case "welkom":
    case "einde":
      return `<div class="head"><h1>${typo(s.header)}</h1><p>${typo(s.sub)}</p></div>
        <ul>${(s.list || []).concat(s.lines || []).map((l, i) => `<li class="${i === s.active ? "now" : ""}">${typo(l)}</li>`).join("")}</ul>
        <p class="footer">${typo(s.footer)}</p>`;
    case "catechismus":
      return bar + `<div class="body"><div class="fit">${(s.lines || []).map((l) => `<p>${typo(l)}</p>`).join("")}</div></div>`;
    case "slotzang":
      if (!s.lines || !s.lines.length) {
        return `<div class="announce"><small>${typo(s.header)}</small><strong>${typo(s.sub)}</strong></div>`;
      }
    // fall through: verse slides of the slotzang look like a psalm, on the dark screen
    case "psalm":
      return bar + `<div class="body"><div class="fit">${(s.lines || []).map((l) => `<div>${typo(l)}</div>`).join("")}</div></div>`;
    case "lezing":
      return bar + `<div class="body"><div class="fit">${(s.verses || []).map((v) => `<p><sup>${v.verse}</sup>${typo(v.text)}</p>`).join("")}${(s.lines || []).map(esc).join("<br>")}</div></div>`;
    case "preek": {
      const verse = (s.verses || []).map((v) => typo(v.text)).join(" ");
      const points = (s.points || []).map((p) => `<li>${typo(p)}</li>`).join("");
      return bar + `<div class="body"><div class="fit">
        ${verse ? `<div class="verse">${verse}</div>` : ""}
        ${(s.lines || []).map((l) => `<div>${typo(l)}</div>`).join("")}
        ${s.theme ? `<div class="theme">${typo(s.theme)}</div>` : ""}
        ${points ? `<ol>${points}</ol>` : ""}</div></div>`;
    }
    default:
      return bar + `<div class="body"><div class="fit">${(s.lines || []).map(esc).join("<br>")}</div></div>`;
  }
}

// renderSlide draws a slide into a .canvas element and scales it to its parent.
// layout: { portrait: board hangs upright (1080x1920), rotate: 0/90/270 degrees for a
// TV that stands upright but still sends a landscape picture }.
function renderSlide(canvas, slide, black, layout) {
  layout = layout || {};
  // One of the church's own screens takes the place of the automatic board.
  var own = !black && slide ? layoutForMoment("bord", slide.kind) : null;
  if (own) return renderLayout(canvas, own, slideFields(slide), layout);
  canvas.style.background = "";
  canvas.className = "canvas " + (layout.portrait ? "portrait " : "") + (slide ? slide.kind : "") + (black ? " black" : "");
  canvas.innerHTML = slideHTML(slide);
  scaleCanvas(canvas, layout);
  fitText(canvas);
  fitRows(canvas);
}

// The psalm board: at least 11 ledges, closer together when there are more lines, and
// a line that is too long gets smaller letters.
function fitRows(canvas) {
  var rows = canvas.querySelectorAll(".bord .row, .vrij .row");
  if (!rows.length) return;
  var n = Math.max(11, rows.length);
  canvas.style.setProperty("--row", Math.floor((1920 - 140) / n) + "px");
  for (var i = 0; i < rows.length; i++) {
    var r = rows[i], size = parseFloat(getComputedStyle(r).fontSize);
    while (size > 40 && r.scrollWidth > r.clientWidth + 1) {
      size -= 4;
      r.style.fontSize = size + "px";
    }
  }
}

function scaleCanvas(canvas, layout) {
  layout = layout || {};
  var w = layout.portrait ? 1080 : 1920, h = layout.portrait ? 1920 : 1080;
  var rot = layout.rotate || 0;
  var box = canvas.parentElement.getBoundingClientRect();
  var turned = rot === 90 || rot === 270;
  var k = Math.min((turned ? box.height : box.width) / w, (turned ? box.width : box.height) / h);
  canvas.style.width = w + "px";
  canvas.style.height = h + "px";
  canvas.style.transform = "translate(" + box.width / 2 + "px," + box.height / 2 + "px) rotate(" + rot + "deg) scale(" + k + ") translate(" + (-w / 2) + "px," + (-h / 2) + "px)";
}

// fitText shrinks the text until it fits the slide body.
function fitText(canvas) {
  const fit = canvas.querySelector(".fit");
  const body = canvas.querySelector(".body");
  if (!fit || !body) return;
  let size = 84;
  fit.style.fontSize = size + "px";
  const room = () => body.clientHeight - 100;
  while (size > 28 && (fit.scrollHeight > room() || fit.scrollWidth > fit.clientWidth + 1)) {
    size -= 2;
    fit.style.fontSize = size + "px";
  }
}

// listen calls onState with every state change from the server and reconnects by itself.
// window.sbConnected tells whether the live connection is up.
// With ?static in the URL it loads the state once (for screenshots).
function listen(onState) {
  var got = function (st) {
    window.sbState = st;
    boardBackground(st.boardBackground);
    onState(st);
  };
  if (location.search.indexOf("static") >= 0) {
    fetch("/api/state").then(function (r) { return r.json(); }).then(got);
    return;
  }
  var es = new EventSource("/events" + (window.sbRole ? "?rol=" + window.sbRole : ""));
  es.onopen = function () { window.sbConnected = true; };
  es.onmessage = function (e) { got(JSON.parse(e.data)); };
  es.onerror = function () {
    window.sbConnected = false;
    es.close();
    setTimeout(function () { listen(onState); }, 2000);
  };
}

// The boards' background: the church's own picture when one is chosen (version > 0),
// otherwise the wood. A new version makes every screen load the picture again.
var boardBackgroundVersion = -1;
function boardBackground(version) {
  version = version || 0;
  if (version === boardBackgroundVersion) return;
  boardBackgroundVersion = version;
  var root = document.documentElement.style;
  root.setProperty("--board-bg", 'url("/bord-achtergrond?v=' + version + '")');
  root.setProperty("--board-shade", version ? ".45" : "0");
}

// ---- The church's own screens: boxes at a place on the picture (see layout.rs). ----

function findLayout(id) {
  var all = (window.sbState && window.sbState.layouts) || [];
  for (var i = 0; i < all.length; i++) if (id && all[i].id === id) return all[i];
  return null;
}

function layoutForMoment(target, moment) {
  var all = (window.sbState && window.sbState.layouts) || [];
  for (var i = 0; i < all.length; i++) if (moment && all[i].target === target && all[i].moment === moment) return all[i];
  return null;
}

var LONG_DAYS = ["Zondag", "Maandag", "Dinsdag", "Woensdag", "Donderdag", "Vrijdag", "Zaterdag"];
var LONG_MONTHS = ["januari", "februari", "maart", "april", "mei", "juni", "juli", "augustus", "september", "oktober", "november", "december"];

// The texts of a board slide for the fields of an own screen: {kop} {onder} {tekst} {voet},
// and of the service {datum} {dienst} {tijd} {predikant}.
function slideFields(s) {
  s = s || {};
  var st = window.sbState || {}, svc = null, i;
  for (i = 0; i < (st.services || []).length; i++) if (st.services[i].id === st.current) svc = st.services[i];
  var lines = (s.list || []).concat(s.lines || []);
  if (s.kind === "thema") lines = (s.points || []).map(function (p, n) { return (n + 1) + ". " + p; });
  var f = {
    kop: s.kind === "thema" ? s.theme : s.header,
    onder: s.sub,
    tekst: lines.join("\n"),
    voet: s.footer,
    datum: "", dienst: "", tijd: "", predikant: "",
  };
  if (svc) {
    var d = new Date(svc.date + "T12:00:00");
    f.datum = isNaN(+d) ? svc.date : LONG_DAYS[d.getDay()] + " " + d.getDate() + " " + LONG_MONTHS[d.getMonth()] + " " + d.getFullYear();
    f.dienst = svc.label;
    f.tijd = svc.time;
    f.predikant = svc.predikant;
  }
  return f;
}

function fillFields(text, f) {
  return String(text == null ? "" : text).replace(/\{(\w+)\}/g, function (m, k) { return f && f[k] != null ? f[k] : f && k in f ? "" : m; });
}

function rgba(hex, opacity) {
  var h = String(hex || "").replace("#", "");
  if (h.length === 3) h = h[0] + h[0] + h[1] + h[1] + h[2] + h[2];
  if (!/^[0-9a-fA-F]{6}$/.test(h)) return "transparent";
  var n = parseInt(h, 16), a = opacity == null ? 1 : Math.max(0, Math.min(1, opacity));
  return "rgba(" + (n >> 16) + "," + ((n >> 8) & 255) + "," + (n & 255) + "," + a + ")";
}

function boxHTML(b, f) {
  var st = "left:" + b.x + "px;top:" + b.y + "px;width:" + b.w + "px;height:" + b.h + "px;";
  st += b.round ? "border-radius:50%;" : b.radius ? "border-radius:" + b.radius + "px;" : "";
  var open = '<div class="lbox l' + esc(b.kind) + '" data-box="' + esc(b.id) + '" style="' + st;
  switch (b.kind) {
    case "vlak":
      return open + "background:" + rgba(b.fill || "#000000", b.opacity) + '"></div>';
    case "afbeelding":
      return open + '">' + (b.image ? '<img src="/afbeelding/' + encodeURIComponent(b.image) + '" style="object-fit:' + (b.fit === "passend" ? "contain" : "cover") + '">' : "") + "</div>";
    case "qr":
      return open + '"><img src="/qr.png"></div>';
    // The camera only exists in the stream; the editor shows where it comes.
    case "camera":
      return open + '"><span>camera</span></div>';
    default: {
      var pad = b.fill ? Math.min(24, b.w / 8, b.h / 8) : 0;
      var just = b.valign === "midden" ? "center" : b.valign === "onder" ? "flex-end" : "flex-start";
      var align = b.align === "midden" ? "center" : b.align === "rechts" ? "right" : "left";
      return open + "padding:" + pad + "px;justify-content:" + just + ";text-align:" + align + ";color:" + esc(b.color || "#ffffff") + ";" +
        (b.fill ? "background:" + rgba(b.fill, b.opacity) + ";" : "") +
        "font-family:" + (b.font === "serif" ? "'Source Serif 4',Georgia,serif" : "'Source Sans 3',Arial,sans-serif") + ";font-weight:" + (b.bold ? 600 : 400) + '">' +
        '<div class="ltext" data-size="' + (+b.size || 60) + '">' + typo(fillFields(b.text, f)) + "</div></div>";
    }
  }
}

// Draws an own screen into a .canvas element, scaled to its parent like renderSlide.
// layout: { portrait, rotate } as there; fields: the texts for {kop} and the others.
function renderLayout(canvas, own, fields, layout) {
  layout = layout || {};
  var portrait = own.target === "bord";
  canvas.className = "canvas own " + (portrait ? "portrait" : "");
  canvas.style.background = own.background || (portrait ? "" : "#000");
  canvas.innerHTML = (own.boxes || []).map(function (b) { return boxHTML(b, fields); }).join("");
  scaleCanvas(canvas, { portrait: portrait, rotate: layout.rotate });
  fitBoxes(canvas);
  // Measured again once the letters are loaded: before that the text is set in another letter.
  if (document.fonts && document.fonts.status !== "loaded" && document.fonts.ready) {
    document.fonts.ready.then(function () { fitBoxes(canvas); });
  }
}

// Text in a box gets smaller until it fits (never larger than the size set for the box).
function fitBoxes(canvas) {
  var texts = canvas.querySelectorAll(".ltext");
  for (var i = 0; i < texts.length; i++) {
    var t = texts[i], box = t.parentElement, size = parseFloat(t.getAttribute("data-size")) || 60;
    var cs = getComputedStyle(box), room = box.clientHeight - parseFloat(cs.paddingTop) - parseFloat(cs.paddingBottom);
    t.style.fontSize = size + "px";
    while (size > 14 && (t.scrollHeight > room + 1 || t.scrollWidth > t.clientWidth + 1)) {
      size = Math.max(14, Math.floor(size * 0.95));
      t.style.fontSize = size + "px";
    }
  }
}
