// EMBEDER desktop — drives the verification loop and renders the instrument panel.
// Works inside the Tauri shell (invoke) or the dev server (fetch).

const $ = (id) => document.getElementById(id);
const STAGES = ["intent", "ground", "compile", "simulate", "verified"];

async function runVerification() {
  const tauri = window.__TAURI__?.core?.invoke;
  if (tauri) return await window.__TAURI__.core.invoke("run_verification");
  const res = await fetch("/api/run");
  if (!res.ok) throw new Error("server " + res.status);
  return await res.json();
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const nodeEl = (stage) => document.querySelector(`.node[data-stage="${stage}"]`);

function resetPipeline() {
  STAGES.forEach((s) => nodeEl(s).classList.remove("flow", "done"));
}

function setRig(state, text) {
  const rig = document.querySelector(".rig-status");
  rig.classList.remove("busy", "ok", "err");
  if (state) rig.classList.add(state);
  $("rigStatus").textContent = text;
}

async function animateAndRun() {
  const btn = $("runBtn");
  btn.disabled = true;
  resetPipeline();
  $("verdictLive").hidden = true;
  $("verdictIdle").hidden = false;
  $("verdictIdle").textContent = "running…";
  setRig("busy", "VERIFYING…");

  // Fire the request while we animate the first stages.
  const pending = runVerification();

  const preStages = ["intent", "ground", "compile", "simulate"];
  for (const s of preStages) {
    const n = nodeEl(s);
    n.classList.add("flow");
    await sleep(420);
    n.classList.remove("flow");
    n.classList.add("done");
  }

  let data;
  try {
    data = await pending;
  } catch (e) {
    setRig("err", "SERVER ERROR");
    $("verdictIdle").textContent = "error: " + e.message;
    btn.disabled = false;
    return;
  }

  if (data.error) {
    setRig("err", "ERROR");
    $("verdictIdle").textContent = data.error;
    btn.disabled = false;
    return;
  }

  const verified = data.state === "verified";
  const term = nodeEl("verified");
  term.classList.add(verified ? "done" : "flow");

  render(data);
  setRig(verified ? "ok" : "err", verified ? "VERIFIED" : data.state.toUpperCase());
  btn.disabled = false;
}

function render(d) {
  $("verdictIdle").hidden = true;
  $("verdictLive").hidden = false;

  const tier = d.tier || { symbol: "[X]", name: d.state };
  const badge = $("tierBadge");
  badge.className = "tier " + tier.name;
  $("tierSym").textContent = tier.symbol;
  $("tierName").textContent = tier.name.replace(/_/g, " ");

  $("attempts").textContent = d.attempts;
  $("healed").innerHTML = d.self_healed
    ? '<span class="healed">↻ self-healed</span>'
    : '<span class="dim">clean first pass</span>';

  // observations
  const obs = $("observations");
  obs.innerHTML = "";
  (d.observations || []).forEach((o) => {
    const el = document.createElement("span");
    el.className = "ob";
    el.innerHTML = `${o.key} <b>${escapeHtml(o.value)}</b>`;
    obs.appendChild(el);
  });

  // boundary
  fillChips("bVerified", d.boundary?.verified);
  fillChips("bStubbed", d.boundary?.stubbed);
  fillChips("bOos", d.boundary?.not_modeled);

  // citations
  const list = $("citeList");
  list.innerHTML = "";
  const cites = d.citations || [];
  $("citeCount").textContent = cites.length ? `${cites.length} facts` : "";
  if (!cites.length) {
    list.innerHTML = '<li class="cite-empty mono">no facts</li>';
  } else {
    cites.forEach((c) => {
      const li = document.createElement("li");
      li.className = "cite";
      li.innerHTML =
        `<span class="c-sum">${escapeHtml(c.summary)}</span>` +
        `<span class="c-loc">${escapeHtml(c.source)} :: ${escapeHtml(c.locator)}</span>`;
      list.appendChild(li);
    });
  }

  // provenance
  const led = $("ledger");
  led.innerHTML = "";
  (d.provenance || []).forEach((r) => {
    const row = document.createElement("div");
    row.className = "prow";
    const tierTxt = r.tier ? r.tier : "—";
    row.innerHTML =
      `<span class="p-id">${escapeHtml(r.id.slice(0, 8))}</span>` +
      `<span class="p-actor">${escapeHtml(r.actor)}</span>` +
      `<span class="p-tool ${escapeHtml(r.tool)}">${escapeHtml(r.tool)}</span>` +
      `<span class="p-tier ${r.tier ? "" : "none"}">${escapeHtml(tierTxt)}</span>`;
    led.appendChild(row);
  });

  $("footArtifact").textContent = d.artifact ? "artifact · " + d.artifact : "";
}

function fillChips(id, items) {
  const el = $(id);
  el.innerHTML = "";
  (items || []).forEach((t) => {
    const c = document.createElement("span");
    c.className = "chip";
    c.textContent = t;
    el.appendChild(c);
  });
}

function escapeHtml(s) {
  return String(s).replace(/[&<>"]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c]));
}

$("runBtn").addEventListener("click", animateAndRun);
