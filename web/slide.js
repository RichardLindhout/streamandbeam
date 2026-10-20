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
  canvas.className = "canvas " + (layout.portrait ? "portrait " : "") + (slide ? slide.kind : "") + (black ? " black" : "");
  canvas.innerHTML = slideHTML(slide);
  scaleCanvas(canvas, layout);
  fitText(canvas);
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
  if (location.search.indexOf("static") >= 0) {
    fetch("/api/state").then(function (r) { return r.json(); }).then(onState);
    return;
  }
  var es = new EventSource("/events");
  es.onopen = function () { window.sbConnected = true; };
  es.onmessage = function (e) { onState(JSON.parse(e.data)); };
  es.onerror = function () {
    window.sbConnected = false;
    es.close();
    setTimeout(function () { listen(onState); }, 2000);
  };
}
