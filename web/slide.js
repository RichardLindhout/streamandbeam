// Shared by the screens (beam.html) and the preview in the operator page.
// Keep this to syntax that older smart-TV browsers understand (no ?. or ??).

function esc(s) {
  return String(s == null ? "" : s).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c]);
}

function slideHTML(s) {
  if (!s) return "";
  const bar = `<div class="bar"><h1>${esc(s.header)}</h1><span>${esc(s.sub)}</span></div>`;
  switch (s.kind) {
    case "welkom":
      return `<div class="head"><h1>${esc(s.header)}</h1><p>${esc(s.sub)}</p></div>
        <ul>${(s.list || []).map((l) => `<li>${esc(l)}</li>`).join("")}</ul>`;
    case "slotzang":
      if (!s.lines || !s.lines.length) {
        return `<div class="announce"><small>${esc(s.header)}</small><strong>${esc(s.sub)}</strong></div>`;
      }
    // fall through: verse slides of the slotzang look like a psalm, on the dark screen
    case "psalm":
      return bar + `<div class="body"><div class="fit">${(s.lines || []).map((l) => `<div>${esc(l)}</div>`).join("")}</div></div>`;
    case "lezing":
      return bar + `<div class="body"><div class="fit">${(s.verses || []).map((v) => `<p><sup>${v.verse}</sup>${esc(v.text)}</p>`).join("")}${(s.lines || []).map(esc).join("<br>")}</div></div>`;
    case "preek": {
      const verse = (s.verses || []).map((v) => esc(v.text)).join(" ");
      const points = (s.points || []).map((p) => `<li>${esc(p)}</li>`).join("");
      return bar + `<div class="body"><div class="fit">
        ${verse ? `<div class="verse">${verse}</div>` : ""}
        ${s.theme ? `<div class="theme">${esc(s.theme)}</div>` : ""}
        ${points ? `<ol>${points}</ol>` : ""}</div></div>`;
    }
    default:
      return bar + `<div class="body"><div class="fit">${(s.lines || []).map(esc).join("<br>")}</div></div>`;
  }
}

// renderSlide draws a slide into a .canvas element and scales it to its parent.
function renderSlide(canvas, slide, black) {
  canvas.className = "canvas " + (slide ? slide.kind : "") + (black ? " black" : "");
  canvas.innerHTML = slideHTML(slide);
  scaleCanvas(canvas);
  fitText(canvas);
}

function scaleCanvas(canvas) {
  const box = canvas.parentElement.getBoundingClientRect();
  const k = Math.min(box.width / 1920, box.height / 1080);
  canvas.style.transform = `translate(${(box.width - 1920 * k) / 2}px, ${(box.height - 1080 * k) / 2}px) scale(${k})`;
}

// fitText shrinks the text until it fits the slide body.
function fitText(canvas) {
  const fit = canvas.querySelector(".fit");
  const body = canvas.querySelector(".body");
  if (!fit || !body) return;
  let size = 84;
  fit.style.fontSize = size + "px";
  const room = () => body.clientHeight - 100;
  while (size > 28 && (fit.scrollHeight > room() || fit.scrollWidth > body.clientWidth - 220)) {
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
