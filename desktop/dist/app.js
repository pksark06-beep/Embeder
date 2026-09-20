// SPDX-License-Identifier: MPL-2.0
const $ = (id) => document.getElementById(id);
const $$ = (selector, root = document) => [...root.querySelectorAll(selector)];
const STAGES = ["intent", "ground", "compile", "simulate", "verified"];

const state = {
  page: "editor",
  project: null,
  workspace: [],
  openFiles: [],
  activeFile: null,
  files: {},
  dirty: new Set(),
  peripherals: [],
  mcp: null,
  verification: null,
  busy: false,
  problems: [],
  panelOpen: true,
  sidebarOpen: true,
  auxOpen: true,
  splitEditor: false,
};

let toastTimer;

const escapeHtml = (value) => String(value ?? "").replace(/[&<>"']/g, (character) => ({
  "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#039;",
}[character]));
const sleep = (milliseconds) => new Promise((resolve) => setTimeout(resolve, milliseconds));

const transport = {
  async call(command, args = {}) {
    const invoke = window.__TAURI__?.core?.invoke;
    if (invoke) {
      const data = await invoke(command, args);
      if (data?.error) throw new Error(data.error);
      return data;
    }

    const routes = {
      project: { url: "/api/project" },
      peripherals: { url: "/api/peripherals" },
      register_map: { url: `/api/register_map?peripheral=${encodeURIComponent(args.peripheral || "")}` },
      firmware: { url: "/api/firmware" },
      workspace_files: { url: "/api/workspace" },
      read_workspace_file: { url: `/api/file?path=${encodeURIComponent(args.path || "")}` },
      save_workspace_file: { url: `/api/file?path=${encodeURIComponent(args.path || "")}`, method: "POST", body: args.contents || "" },
      mcp_status: { url: "/api/mcp" },
      probe_mcp: { url: `/api/mcp_probe?server=${encodeURIComponent(args.server || "")}` },
      start_task: { url: `/api/tasks?simulate=${args.simulate !== false}`, method: "POST" },
      generate: { url: "/api/generate", method: "POST", body: args.goal || "" },
      task_status: { url: `/api/task?id=${encodeURIComponent(args.id)}` },
    };
    const route = routes[command];
    if (!route) throw new Error(`No browser route for ${command}`);
    const response = await fetch(route.url, {
      method: route.method || "GET",
      body: route.body,
      headers: { Accept: "application/json", ...(route.method ? { "Content-Type": "text/plain; charset=utf-8", "X-Embeder-Client": "1" } : {}) },
    });
    if (!response.ok) throw new Error(`${command} returned HTTP ${response.status}`);
    const data = await response.json();
    if (data?.error) throw new Error(data.error);
    return data;
  },
};

function setPage(page) {
  const target = document.querySelector(`.page[data-page="${page}"]`);
  if (!target) return;
  state.page = page;
  $$(".page").forEach((section) => { section.hidden = section !== target; });
  $$(".activity-item").forEach((button) => {
    const active = button.dataset.page === page;
    button.classList.toggle("active", active);
    button.setAttribute("aria-current", active ? "page" : "false");
  });
  renderSidebar();
}

function setConsole(name) {
  $$(".panel-tab").forEach((button) => button.classList.toggle("active", button.dataset.console === name));
  $$(".panel-output").forEach((output) => { output.hidden = output.dataset.console !== name; });
}

function togglePanel(force) {
  state.panelOpen = typeof force === "boolean" ? force : !state.panelOpen;
  $("bottomPanel").classList.toggle("collapsed", !state.panelOpen);
  document.body.classList.toggle("panel-collapsed", !state.panelOpen);
}

function toggleSidebar(force) {
  state.sidebarOpen = typeof force === "boolean" ? force : !state.sidebarOpen;
  document.body.classList.toggle("sidebar-collapsed", !state.sidebarOpen);
  const button = $("toggleSidebar");
  if (button) {
    button.title = state.sidebarOpen ? "Collapse Explorer" : "Show Explorer";
    button.innerHTML = `<svg><use href="#${state.sidebarOpen ? "i-close" : "i-files"}"/></svg>`;
  }
}

function toggleAux(force) {
  state.auxOpen = typeof force === "boolean" ? force : !state.auxOpen;
  document.body.classList.toggle("aux-collapsed", !state.auxOpen);
  const button = $("toggleAux");
  if (button) {
    button.title = state.auxOpen ? "Collapse context" : "Show context";
    button.innerHTML = `<svg><use href="#${state.auxOpen ? "i-close" : "i-more"}"/></svg>`;
  }
}

function syncSplitPane() {
  const grid = $("editorPaneGrid");
  const primary = $("editorSurface");
  if (!grid || !primary) return;
  const secondary = grid.querySelector(".split-secondary");
  if (!state.splitEditor) {
    grid.classList.remove("split");
    secondary?.remove();
    $("splitEditor").classList.remove("active");
    $("splitEditor").title = "Split editor";
    return;
  }
  grid.classList.add("split");
  $("splitEditor").classList.add("active");
  $("splitEditor").title = "Close split editor";
  if (secondary) return;
  const clone = primary.cloneNode(true);
  clone.classList.add("split-secondary");
  clone.querySelectorAll("[id]").forEach((node) => { node.id = `${node.id}-secondary`; });
  const editor = clone.querySelector(".code-input");
  if (editor) {
    editor.readOnly = true;
    editor.setAttribute("aria-label", "Read-only split preview");
    editor.tabIndex = -1;
  }
  grid.appendChild(clone);
}

function refreshSplitPreview() {
  const secondary = $("editorPaneGrid")?.querySelector(".split-secondary");
  if (!secondary) return;
  const primary = $("editorSurface");
  const source = primary.querySelector(".code-input").value;
  secondary.querySelector(".code-input").value = source;
  secondary.querySelector(".code-highlight").innerHTML = primary.querySelector(".code-highlight").innerHTML;
  secondary.querySelector(".line-numbers").innerHTML = primary.querySelector(".line-numbers").innerHTML;
  secondary.querySelector(".minimap").textContent = source;
}

function log(target, message, kind = "info", source = "workbench") {
  const output = $(target);
  if (!output) return;
  const row = document.createElement("div");
  row.className = `log-line ${kind}`;
  const timestamp = new Date().toLocaleTimeString([], { hour12: false });
  row.innerHTML = `<time>${timestamp}</time><b>${escapeHtml(source)}</b><span>${escapeHtml(message)}</span>`;
  output.appendChild(row);
  output.scrollTop = output.scrollHeight;
}

function showToast(message, kind = "ok") {
  clearTimeout(toastTimer);
  $("toastText").textContent = message;
  const dot = $("toast").querySelector(".status-dot");
  dot.className = `status-dot ${kind === "error" ? "offline" : kind === "warn" ? "checking" : ""}`;
  $("toast").hidden = false;
  toastTimer = setTimeout(() => { $("toast").hidden = true; }, 2600);
}

function reportProblem(message) {
  state.problems.push(message);
  $("problemCount").textContent = state.problems.length;
  $("sbProblems").textContent = `× ${state.problems.length}`;
  log("conProblems", message, "error", "error");
}

function renderSidebar() {
  const titles = {
    editor: "EXPLORER",
    verify: "VERIFICATION",
    datasheet: "DATASHEET",
    simulation: "SIMULATION",
    extensions: "EXTENSIONS",
    guide: "GETTING STARTED",
    settings: "SETTINGS",
  };
  $("sideTitle").textContent = titles[state.page] || state.page.toUpperCase();

  if (state.page === "editor") {
    const open = state.openFiles.map((path) => `
      <li class="${path === state.activeFile ? "active" : ""}" data-open-file="${escapeHtml(path)}"><span class="file-icon">${fileGlyph(path)}</span><span>${escapeHtml(fileName(path))}</span></li>`).join("");
    const groups = groupFiles(state.workspace);
    $("sideContent").innerHTML = `
      <section class="side-section"><div class="side-section-title">Open editors</div><ul class="file-tree">${open || '<li class="empty-tree">No open files</li>'}</ul></section>
      <section class="side-section"><div class="side-section-title">STM32-BLINK-UART</div>${groups}</section>
      <div class="side-note">Files are read and written only inside the firmware workspace jail.</div>`;
  } else if (state.page === "verify") {
    const result = state.verification;
    $("sideContent").innerHTML = `
      <section class="side-section"><div class="side-section-title">Run</div>
        <div class="side-stat"><span>Verdict</span><b>${escapeHtml(result?.state || "not run")}</b></div>
        <div class="side-stat"><span>Mode</span><b>${escapeHtml(result?.mode || "real")}</b></div>
        <div class="side-stat"><span>Attempts</span><b>${escapeHtml(result?.attempts ?? "—")}</b></div>
      </section>
      <section class="side-section"><div class="side-section-title">Pipeline</div>${STAGES.map((stage) => `<div class="side-stat"><span>${stage}</span><b>${result ? "ready" : "waiting"}</b></div>`).join("")}</section>`;
  } else if (state.page === "datasheet") {
    $("sideContent").innerHTML = `<section class="side-section"><div class="side-section-title">Structured source</div><div class="side-stat"><span>Device</span><b>STM32F407</b></div><div class="side-stat"><span>Peripherals</span><b>${state.peripherals.length}</b></div><div class="side-stat"><span>Source</span><b>CMSIS-SVD</b></div></section><div class="side-note">A miss is refused and never replaced with a model guess.</div>`;
  } else if (state.page === "simulation") {
    $("sideContent").innerHTML = `<section class="side-section"><div class="side-section-title">Engine</div><div class="side-stat"><span>Runtime</span><b>Renode</b></div><div class="side-stat"><span>UART</span><b>USART2</b></div><div class="side-stat"><span>Platform</span><b>STM32F4</b></div></section><div class="side-note">Analog values are stubbed. RF, Wi-Fi, and BLE are outside the modeled boundary.</div>`;
  } else if (state.page === "extensions") {
    const servers = state.mcp?.servers || [];
    $("sideContent").innerHTML = `<section class="side-section"><div class="side-section-title">MCP servers</div>${servers.map((server) => `<div class="side-stat"><span>${escapeHtml(server.name)}</span><b>${escapeHtml(server.status)}</b></div>`).join("") || '<div class="side-note">Discovering servers…</div>'}</section><div class="side-note">Each tool request starts a dedicated worker process over stdio JSON-RPC.</div>`;
  } else if (state.page === "guide") {
    $("sideContent").innerHTML = `<section class="side-section"><div class="side-section-title">Workflow</div><div class="side-stat"><span>1. Edit</span><b>Explorer</b></div><div class="side-stat"><span>2. Build</span><b>GCC</b></div><div class="side-stat"><span>3. Simulate</span><b>Renode</b></div></section><div class="side-note">The guide stays available from the question-mark icon whenever you need it.</div>`;
  } else {
    $("sideContent").innerHTML = `<section class="side-section"><div class="side-section-title">Categories</div><div class="side-stat"><span>Toolchains</span><b>Local</b></div><div class="side-stat"><span>Security</span><b>Path checks</b></div><div class="side-stat"><span>Providers</span><b>Not configured</b></div></section>`;
  }

  $$('[data-open-file]', $("sideContent")).forEach((item) => item.addEventListener("click", () => openFile(item.dataset.openFile)));
}

function groupFiles(files) {
  const groups = new Map();
  files.forEach((file) => {
    const parts = file.path.split("/");
    const folder = parts.length > 1 ? parts.slice(0, -1).join("/") : "root";
    if (!groups.has(folder)) groups.set(folder, []);
    groups.get(folder).push(file);
  });
  return [...groups.entries()].map(([folder, entries]) => `
    <ul class="file-tree"><li class="folder-row">${escapeHtml(folder)}</li>${entries.map((file) => `
      <li class="${file.path === state.activeFile ? "active" : ""}" data-open-file="${escapeHtml(file.path)}"><span class="file-icon">${fileGlyph(file.path)}</span><span>${escapeHtml(file.name)}</span></li>`).join("")}</ul>`).join("");
}

function fileName(path) { return String(path || "").split("/").pop(); }
function fileGlyph(path) {
  if (path.endsWith(".c") || path.endsWith(".h")) return "C";
  if (path.endsWith(".ld")) return "LD";
  if (path.endsWith(".s") || path.endsWith(".S")) return "S";
  return "·";
}

async function openFile(path) {
  if (!path) return;
  try {
    if (!state.files[path]) {
      const file = await transport.call("read_workspace_file", { path });
      state.files[path] = { ...file, saved: file.contents };
    }
    if (!state.openFiles.includes(path)) state.openFiles.push(path);
    state.activeFile = path;
    setPage("editor");
    renderEditor();
    renderFileContext(state.files[path]);
    log("conBuild", `Opened ${path} inside the workspace jail`, "info", "editor");
  } catch (error) {
    reportProblem(error.message);
    showToast(error.message, "error");
  }
}

function closeFile(path, event) {
  event?.stopPropagation();
  const index = state.openFiles.indexOf(path);
  if (index < 0) return;
  state.openFiles.splice(index, 1);
  if (state.activeFile === path) {
    state.activeFile = state.openFiles[Math.max(0, index - 1)] || state.openFiles[0] || null;
  }
  renderEditor();
  renderSidebar();
}

function renderEditor() {
  $("editorTabs").innerHTML = state.openFiles.map((path) => `
    <button class="editor-tab ${path === state.activeFile ? "active" : ""}" data-tab-file="${escapeHtml(path)}">
      <span class="file-icon">${fileGlyph(path)}</span><span class="tab-name">${escapeHtml(fileName(path))}</span>
      ${state.dirty.has(path) ? '<i class="dirty" title="Unsaved"></i>' : '<span class="tab-close" data-close-file="' + escapeHtml(path) + '"><svg><use href="#i-close"/></svg></span>'}
    </button>`).join("");
  $$('[data-tab-file]', $("editorTabs")).forEach((tab) => tab.addEventListener("click", () => openFile(tab.dataset.tabFile)));
  $$('[data-close-file]', $("editorTabs")).forEach((close) => close.addEventListener("click", (event) => closeFile(close.dataset.closeFile, event)));

  const file = state.activeFile ? state.files[state.activeFile] : null;
  if (!file) {
    $("activeBreadcrumb").textContent = "No file open";
    $("codeEditor").value = "";
    renderCodeSurface();
    return;
  }
  $("activeBreadcrumb").textContent = state.activeFile;
  $("codeEditor").readOnly = file.editable === false;
  $("codeEditor").value = file.contents;
  $("languageMode").textContent = String(file.language || "text").toUpperCase();
  renderCodeSurface();
}

function renderCodeSurface() {
  const editor = $("codeEditor");
  const source = editor.value;
  $("codeHighlight").innerHTML = highlightSource(source) + "\n";
  const lineCount = Math.max(1, source.split("\n").length);
  const currentLine = source.slice(0, editor.selectionStart).split("\n").length;
  $("lineNumbers").innerHTML = Array.from({ length: lineCount }, (_, index) => `<span class="${index + 1 === currentLine ? "current" : ""}">${index + 1}</span>`).join("");
  $("minimap").textContent = source;
  updateCursor();
  syncEditorScroll();
  refreshSplitPreview();
}

function highlightSource(source) {
  const tokenPattern = /(\/\/[^\n]*|"(?:\\.|[^"\\])*"|'(?:\\.|[^'\\])*'|^\s*#[^\n]*|\b(?:const|static|volatile|void|int|char|return|while|for|if|else|typedef|struct|uint\d+_t)\b|\b(?:0x[0-9A-Fa-f]+|\d+)\b|\b(?:GPIO[A-Z]?|USART\d|RCC|SysTick)[A-Za-z0-9_]*)/gm;
  let output = "";
  let cursor = 0;
  for (const match of source.matchAll(tokenPattern)) {
    output += escapeHtml(source.slice(cursor, match.index));
    const token = match[0];
    let className = "tok-keyword";
    if (token.trimStart().startsWith("//")) className = "tok-comment";
    else if (token.trimStart().startsWith("#")) className = "tok-pre";
    else if (token.startsWith('"') || token.startsWith("'")) className = "tok-string";
    else if (/^(?:0x|\d)/.test(token)) className = "tok-number";
    else if (/^(GPIO|USART|RCC|SysTick)/.test(token)) className = "tok-register";
    output += `<span class="${className}">${escapeHtml(token)}</span>`;
    cursor = match.index + token.length;
  }
  return output + escapeHtml(source.slice(cursor));
}

function syncEditorScroll() {
  const editor = $("codeEditor");
  $("codeHighlight").style.transform = `translate(${-editor.scrollLeft}px, ${-editor.scrollTop}px)`;
  $("lineNumbers").scrollTop = editor.scrollTop;
  $("minimap").scrollTop = editor.scrollTop / 5;
}

function updateCursor() {
  const editor = $("codeEditor");
  const before = editor.value.slice(0, editor.selectionStart);
  const lines = before.split("\n");
  $("cursorPosition").textContent = `Ln ${lines.length}, Col ${lines.at(-1).length + 1}`;
}

function onEditorInput() {
  if (!state.activeFile || !state.files[state.activeFile]) return;
  state.files[state.activeFile].contents = $("codeEditor").value;
  state.dirty.add(state.activeFile);
  if (state.verification) {
    $("verdictMetric").textContent = "Source changed";
    $("verdictMode").textContent = "Previous result applies to an earlier source snapshot";
  }
  renderCodeSurface();
  renderEditorTabsOnly();
}

function renderEditorTabsOnly() {
  const selection = $("codeEditor").selectionStart;
  const scrollTop = $("codeEditor").scrollTop;
  renderEditor();
  $("codeEditor").selectionStart = selection;
  $("codeEditor").selectionEnd = selection;
  $("codeEditor").scrollTop = scrollTop;
  renderCodeSurface();
}

async function saveActiveFile() {
  const path = state.activeFile;
  if (!path || !state.files[path] || !state.dirty.has(path)) {
    showToast(path ? "No changes to save" : "No active file", "warn");
    return true;
  }
  try {
    const contents = state.files[path].contents;
    const result = await transport.call("save_workspace_file", { path, contents });
    state.files[path].saved = contents;
    state.dirty.delete(path);
    renderEditorTabsOnly();
    log("conBuild", `Saved ${result.path} (${result.bytes} bytes)`, "ok", "editor");
    showToast(`Saved ${fileName(path)} inside the workspace jail`);
    return true;
  } catch (error) {
    reportProblem(error.message);
    showToast(error.message, "error");
    return false;
  }
}

function renderFileContext(file) {
  $("inspBody").innerHTML = `
    <div class="context-title">${escapeHtml(file.name)}</div>
    <section class="context-group"><h3>File</h3>
      <div class="context-row"><span>Path</span><b>${escapeHtml(file.path)}</b></div>
      <div class="context-row"><span>Language</span><b>${escapeHtml(file.language)}</b></div>
      <div class="context-row"><span>Size</span><b>${escapeHtml(file.bytes)} bytes</b></div>
      <div class="context-row"><span>File access</span><b>Workspace path checked</b></div>
    </section>
    <section class="context-group"><h3>Verification</h3><div class="evidence-item">Source changes are not trusted until the compiler and simulator oracles pass.<code>Core → MCP → toolchain</code></div></section>`;
}

function renderProject(project) {
  state.project = project;
  $("targetChip").textContent = project.mcu;
  $("toolchains").innerHTML = (project.toolchains || []).map((tool) => `
    <div class="setting-row"><i class="status-dot ${tool.available ? "" : "offline"}"></i><div><b>${escapeHtml(tool.name)}</b><span>${escapeHtml(tool.version || "Not found")}</span></div></div>`).join("");
}

function renderPeripherals(payload) {
  state.peripherals = payload.peripherals || [];
  $("dsCount").textContent = state.peripherals.length;
  $("periphList").innerHTML = state.peripherals.map((peripheral) => `
    <button class="peripheral-item" data-peripheral="${escapeHtml(peripheral.name)}"><b>${escapeHtml(peripheral.name)}</b><small>${formatHex(peripheral.base_address)} · ${peripheral.register_count} registers</small></button>`).join("");
  $$('[data-peripheral]', $("periphList")).forEach((button) => button.addEventListener("click", () => loadRegisterMap(button.dataset.peripheral, button)));
}

function formatHex(value) {
  if (typeof value === "number") return `0x${value.toString(16).toUpperCase().padStart(8, "0")}`;
  return String(value ?? "—");
}

async function loadRegisterMap(name, selected) {
  $$('[data-peripheral]', $("periphList")).forEach((button) => button.classList.toggle("active", button === selected));
  $("dsDetailLabel").textContent = `${name} register map`;
  $("regTable").innerHTML = '<div class="empty-state">Loading grounded register data…</div>';
  try {
    const data = await transport.call("register_map", { peripheral: name });
    const peripheral = data.peripheral;
    $("regTable").innerHTML = `
      <div class="register-head"><span>Register</span><span>Address</span><span>Description</span></div>
      ${(peripheral.registers || []).map((register) => `
        <div class="register-row"><b>${escapeHtml(register.name)}</b><code>${formatHex(register.absolute_address)}</code><span>${escapeHtml(register.description || "—")}</span></div>
        <div class="field-row">${(register.fields || []).map((field) => `<span class="field-chip" title="${escapeHtml(field.description || "")}">${escapeHtml(field.name)} · ${field.bit_offset}:${field.bit_width}</span>`).join("") || '<span class="field-chip">No fields</span>'}</div>`).join("")}`;
    renderRegisterContext(data);
    log("conMcp", `register_map(${name}) returned ${peripheral.registers.length} grounded registers`, "ok", "datasheet");
  } catch (error) {
    $("regTable").innerHTML = `<div class="empty-state">${escapeHtml(error.message)}</div>`;
    reportProblem(error.message);
  }
}

function renderRegisterContext(data) {
  const peripheral = data.peripheral;
  $("inspBody").innerHTML = `
    <div class="context-title">${escapeHtml(peripheral.name)}</div>
    <section class="context-group"><h3>Grounding</h3><div class="context-row"><span>Confidence</span><b>${escapeHtml(data.tier?.symbol)} ${escapeHtml(data.tier?.name)}</b></div><div class="context-row"><span>Base</span><b>${formatHex(peripheral.base_address)}</b></div><div class="context-row"><span>Registers</span><b>${peripheral.registers.length}</b></div></section>
    <section class="context-group"><h3>Evidence</h3><div class="evidence-item">${escapeHtml(data.citation?.path || "Structured source")}<code>${escapeHtml(data.citation?.source || "stm32f4-mini.svd")}</code></div></section>`;
}

async function loadMcp() {
  setMcpIndicator("checking", "MCP checking");
  $("extensionGrid").innerHTML = '<div class="empty-state">Discovering local MCP workers…</div>';
  try {
    state.mcp = await transport.call("mcp_status");
    renderMcp();
    const online = state.mcp.servers.filter((server) => server.status === "online").length;
    setMcpIndicator(online === state.mcp.servers.length ? "online" : "offline", `${online}/${state.mcp.servers.length} MCP online`);
    $("mcpMetric").textContent = `${online}/${state.mcp.servers.length} online`;
    log("conMcp", `Discovered ${online} online MCP servers over stdio`, online ? "ok" : "warn", "mcp");
    renderSidebar();
  } catch (error) {
    setMcpIndicator("offline", "MCP offline");
    $("extensionGrid").innerHTML = `<div class="empty-state">${escapeHtml(error.message)}</div>`;
    reportProblem(`MCP discovery: ${error.message}`);
  }
}

function setMcpIndicator(status, label) {
  $("toolbarMcpDot").className = `status-dot ${status === "checking" ? "checking" : status === "offline" ? "offline" : ""}`;
  $("sbMcp").innerHTML = `<i class="status-dot ${status === "checking" ? "checking" : status === "offline" ? "offline" : ""}"></i>${escapeHtml(label)}`;
}

function renderMcp(filter = "") {
  if (!state.mcp) return;
  const query = filter.trim().toLowerCase();
  const servers = state.mcp.servers.filter((server) => JSON.stringify(server).toLowerCase().includes(query));
  const online = state.mcp.servers.filter((server) => server.status === "online").length;
  $("extensionSummary").textContent = `${online} online · ${state.mcp.servers.reduce((sum, server) => sum + server.tools.length, 0)} tools`;
  $("sandboxSummary").textContent = `${state.mcp.sandbox.process_model}. Network: ${state.mcp.sandbox.network}.`;
  $("extensionGrid").innerHTML = servers.map((server) => {
    const dependencyReady = server.dependency?.available;
    return `<article class="extension-card" data-extension-card="${escapeHtml(server.id)}">
      <div class="extension-icon"><svg><use href="#${server.id === "datasheet" ? "i-chip" : server.id === "simulation" ? "i-wave" : "i-files"}"/></svg></div>
      <div class="extension-main"><div class="extension-head"><h2>${escapeHtml(server.name)} MCP</h2><span class="extension-state ${server.status === "online" ? "" : "offline"}">${escapeHtml(server.status)}</span></div>
      <p>${escapeHtml(server.transport)} · ${escapeHtml(server.isolation)} · ${server.latency_ms} ms</p>
      <div class="extension-meta">${server.tools.map((tool) => `<span>${escapeHtml(tool.name)}</span>`).join("") || '<span>no tools</span>'}</div>
      <div class="extension-tools">${server.tools.map((tool) => `<div class="extension-tool"><code>${escapeHtml(tool.name)}</code><span>${escapeHtml(tool.description)}</span></div>`).join("")}</div>
      <div class="extension-actions"><button data-probe-mcp="${escapeHtml(server.id)}">Check MCP response</button><span class="dependency ${dependencyReady ? "" : "missing"}">${escapeHtml(server.dependency?.name || "runtime")}: ${dependencyReady ? "ready" : "missing"}</span></div>
      </div></article>`;
  }).join("") || '<div class="empty-state">No extensions match this filter.</div>';
  $$('[data-probe-mcp]', $("extensionGrid")).forEach((button) => button.addEventListener("click", () => probeMcp(button.dataset.probeMcp, button)));
}

async function probeMcp(server, button) {
  const original = button.textContent;
  button.disabled = true;
  button.textContent = "Checking…";
  try {
    const result = await transport.call("probe_mcp", { server });
    log("conMcp", `${server}: MCP response received in ${result.latency_ms} ms (${result.tool_count} tools)`, "ok", "mcp");
    showToast(`${server} MCP check passed`);
    setConsole("mcp");
    togglePanel(true);
  } catch (error) {
    reportProblem(`${server} MCP: ${error.message}`);
    showToast(error.message, "error");
  } finally {
    button.disabled = false;
    button.textContent = original;
  }
}

function resetStages() {
  $$(".stage").forEach((stage) => stage.classList.remove("running", "done", "failed"));
}

async function runVerification(options = {}) {
  if (state.busy) return;
  state.busy = true;
  const buttons = ["toolbarBuild", "toolbarRun", "toolbarGenerate", "studioGenerate", "studioBuild", "pageRun", "pageSimulate"].map($);
  buttons.forEach((button) => { if (button) button.disabled = true; });
  resetStages();
  $("reasoning").innerHTML = "";
  $("conBuild").innerHTML = "";
  $("conProblems").innerHTML = "";
  state.problems = [];
  $("problemCount").textContent = "0";
  $("sbProblems").textContent = "× 0";
  $("verdictMetric").textContent = "Running";
  $("verdictMode").textContent = "Real workspace task";
  setPage("verify");
  setConsole("build");
  togglePanel(true);
  try {
    // Save every dirty buffer, including inactive tabs, before snapshotting.
    for (const path of [...state.dirty]) {
      const contents = state.files[path].contents;
      await transport.call("save_workspace_file", { path, contents });
      state.files[path].saved = contents;
      if (state.files[path].contents === contents) state.dirty.delete(path);
      log("conBuild", `Saved ${path}`, "ok", "editor");
    }
    renderEditorTabsOnly();
    const task = await transport.call("start_task", { simulate: options.buildOnly !== true });
    let seenEvents = 0;
    let snapshot;
    do {
      snapshot = await transport.call("task_status", { id: task.id });
      for (const event of (snapshot.events || []).slice(seenEvents)) {
        const stage = document.querySelector(`.stage[data-stage="${event.stage}"]`);
        if (stage) {
          stage.classList.remove("running", "done", "failed");
          stage.classList.add(event.status);
        }
        addReasoning(event.stage, event.message, event.status);
        log("conBuild", event.message, event.status === "failed" ? "error" : event.status === "done" ? "ok" : "info", event.stage);
      }
      seenEvents = (snapshot.events || []).length;
      if (snapshot.status !== "completed") await sleep(250);
    } while (snapshot.status !== "completed");
    const result = snapshot.result;
    if (result.error) throw new Error(result.error);
    state.verification = result;
    renderVerification(result);
    for (const [source, output] of [["gcc stdout", result.compile?.stdout], ["gcc stderr", result.compile?.stderr], ["renode", result.simulation?.log]]) {
      if (output) log("conBuild", output, "info", source);
    }
    if (result.artifact) log("conBuild", `ELF: ${result.artifact}`, "ok", "artifact");
    log("conBuild", `Source ${result.source_fingerprint} · ${result.duration_ms} ms`, "info", "task");
    for (const diagnostic of result.diagnostics || []) {
      const message = `${diagnostic.file || "gcc"}:${diagnostic.line || 0}:${diagnostic.col || 0}: ${diagnostic.message}`;
      reportProblem(message);
      const row = $("conProblems").lastElementChild;
      if (diagnostic.file && state.workspace.some((file) => file.path === diagnostic.file)) {
        row.tabIndex = 0;
        row.setAttribute("role", "button");
        row.title = "Open source at this diagnostic";
        const navigate = async () => {
          await openFile(diagnostic.file);
          setPage("editor");
          const editor = $("codeEditor");
          const lines = editor.value.split("\n");
          const offset = lines.slice(0, Math.max(0, (diagnostic.line || 1) - 1)).reduce((sum, line) => sum + line.length + 1, 0) + Math.max(0, (diagnostic.col || 1) - 1);
          editor.focus();
          editor.setSelectionRange(offset, offset);
          editor.scrollTop = Math.max(0, ((diagnostic.line || 1) - 4) * 20);
          syncEditorScroll();
          updateCursor();
        };
        row.addEventListener("click", navigate);
        row.addEventListener("keydown", (event) => { if (event.key === "Enter") navigate(); });
      }
    }
    if (options.openSimulation) setPage("simulation");
    renderSidebar();
    if (state.dirty.size) {
      $("verdictMetric").textContent = "Source changed";
      $("verdictMode").textContent = "Result applies to the saved source snapshot";
    }
  } catch (error) {
    document.querySelector('.stage[data-stage="verified"]').classList.add("failed");
    $("verdictMetric").textContent = "Task failed";
    reportProblem(error.message);
    addReasoning("error", error.message, "stopped");
    showToast(error.message, "error");
  } finally {
    state.busy = false;
    buttons.forEach((button) => { if (button) button.disabled = false; });
  }
}

async function generateFirmware() {
  if (state.busy) return;
  setPage("verify");
  const input = $("intentInput");
  const goal = (input?.value || "").trim();
  if (!goal) {
    showToast("Describe what you want to build first", "warn");
    input?.focus();
    return;
  }
  state.busy = true;
  const buttons = ["toolbarGenerate", "toolbarBuild", "toolbarRun", "studioGenerate", "studioBuild", "pageRun", "pageSimulate"].map($);
  buttons.forEach((button) => { if (button) button.disabled = true; });
  resetStages();
  const draftCard = $("draftCard");
  if (draftCard) draftCard.hidden = true;
  $("reasoning").innerHTML = "";
  $("conBuild").innerHTML = "";
  $("conProblems").innerHTML = "";
  state.problems = [];
  $("problemCount").textContent = "0";
  $("sbProblems").textContent = "× 0";
  $("verdictMetric").textContent = "Generating";
  $("verdictMode").textContent = "Model drafting firmware";
  setPage("verify");
  setConsole("build");
  togglePanel(true);
  try {
    log("conBuild", `Intent: ${goal}`, "info", "intent");
    const task = await transport.call("generate", { goal });
    let seenEvents = 0;
    let snapshot;
    do {
      snapshot = await transport.call("task_status", { id: task.id });
      for (const event of (snapshot.events || []).slice(seenEvents)) {
        const stage = document.querySelector(`.stage[data-stage="${event.stage}"]`);
        if (stage) {
          stage.classList.remove("running", "done", "failed");
          stage.classList.add(event.status);
        }
        addReasoning(event.stage, event.message, event.status);
        log("conBuild", event.message, event.status === "failed" ? "error" : event.status === "done" ? "ok" : "info", event.stage);
      }
      seenEvents = (snapshot.events || []).length;
      if (snapshot.status !== "completed") await sleep(250);
    } while (snapshot.status !== "completed");
    const result = snapshot.result;
    if (result.error) throw new Error(result.error);
    state.verification = result;

    // Surface the model's draft as an unsaved buffer (generation never writes to disk).
    if (result.drafted_source && result.drafted_entry) {
      const path = result.drafted_entry;
      const previous = state.files[path];
      state.files[path] = {
        path,
        name: fileName(path),
        language: "c",
        editable: true,
        contents: result.drafted_source,
        saved: previous ? previous.saved : "",
      };
      state.dirty.add(path);
      if (!state.openFiles.includes(path)) state.openFiles.push(path);
      state.activeFile = path;
      renderEditor();
      const by = result.drafted_by || {};
      const who = by.used_llm ? `${by.provider}/${by.model}` : "grounded synthesis";
      log("conBuild", `Draft by ${who} loaded into ${path} (review it in Files, or Save to keep it)`, "ok", "draft");
    }

    renderVerification(result);
    showDraft(result);
    for (const [source, output] of [["gcc stderr", result.compile?.stderr], ["renode", result.simulation?.log]]) {
      if (output) log("conBuild", output, "info", source);
    }
    if (result.artifact) log("conBuild", `ELF: ${result.artifact}`, "ok", "artifact");
    log("conBuild", `Source ${result.source_fingerprint} · ${result.duration_ms} ms`, "info", "task");
    renderSidebar();
  } catch (error) {
    document.querySelector('.stage[data-stage="verified"]').classList.add("failed");
    $("verdictMetric").textContent = "Generation failed";
    reportProblem(error.message);
    addReasoning("error", error.message, "stopped");
    showToast(error.message, "error");
  } finally {
    state.busy = false;
    buttons.forEach((button) => { if (button) button.disabled = false; });
  }
}

function showDraft(result) {
  const card = $("draftCard");
  if (!card) return;
  if (!result.drafted_source) { card.hidden = true; return; }
  const by = result.drafted_by || {};
  const who = by.used_llm ? `${by.provider} / ${by.model}` : "grounded synthesis (no API key)";
  $("draftBy").textContent = who;
  $("draftCode").textContent = result.drafted_source;
  card.hidden = false;
  const path = result.drafted_entry || "src/main.c";
  const saveButton = $("draftSave");
  if (saveButton) {
    saveButton.onclick = async () => {
      try {
        await transport.call("save_workspace_file", { path, contents: result.drafted_source });
        if (state.files[path]) { state.files[path].saved = result.drafted_source; state.dirty.delete(path); }
        showToast(`Saved ${path} to the workspace`, "ok");
        renderEditor();
        renderSidebar();
      } catch (error) {
        showToast(error.message, "error");
      }
    };
  }
}

function addReasoning(icon, message, status) {
  const row = document.createElement("div");
  row.className = "reason-row";
  row.innerHTML = `<i>${escapeHtml(icon.slice(0, 2).toUpperCase())}</i><span>${escapeHtml(message)}</span><small>${escapeHtml(status)}</small>`;
  $("reasoning").appendChild(row);
}

function renderVerification(result) {
  const passed = result.state === "verified";
  $("verdictMetric").textContent = passed ? `${result.tier?.symbol || "✓"} Verified` : result.state;
  $("verdictMode").textContent = `${result.mode || "unknown"} · ${result.attempts} attempt${result.attempts === 1 ? "" : "s"}`;
  $("modePill").textContent = String(result.mode || "unknown").toUpperCase();
  $("goal").textContent = result.goal || $("goal").textContent;
  $("simTerm").textContent = (result.observations || []).map((item) => `${item.key}: ${item.value}`).join("\n") || "No simulation observations returned.";
  fillChips("simVerified", result.boundary?.verified || []);
  fillChips("simStubbed", result.boundary?.stubbed || []);
  fillChips("simOos", result.boundary?.not_modeled || []);
  $("conProv").innerHTML = (result.provenance || []).map((record) => `<div class="provenance-row"><span>${escapeHtml(String(record.id).slice(0, 8))}</span><span>${escapeHtml(record.actor)}</span><span>${escapeHtml(record.tool)}</span><span>${escapeHtml(record.oracle || "—")}</span><span>${escapeHtml(record.tier || "—")}</span></div>`).join("");
  (result.observations || []).forEach((item) => log("conBuild", `${item.key}: ${item.value}`, "ok", "simulate"));
  renderVerificationContext(result);
  showToast(passed ? "Verification completed within the stated boundary" : `Verification ${result.state}`, passed ? "ok" : "warn");
}

function fillChips(id, items) {
  $(id).innerHTML = items.map((item) => `<span class="chip">${escapeHtml(item)}</span>`).join("");
}

function renderVerificationContext(result) {
  $("inspBody").innerHTML = `
    <div class="context-title">Latest verification</div>
    <section class="context-group"><h3>Outcome</h3><div class="context-row"><span>State</span><b>${escapeHtml(result.state)}</b></div><div class="context-row"><span>Mode</span><b>${escapeHtml(result.mode)}</b></div><div class="context-row"><span>Attempts</span><b>${escapeHtml(result.attempts)}</b></div><div class="context-row"><span>Artifact</span><b>${escapeHtml(result.artifact || "—")}</b></div></section>
    <section class="context-group"><h3>Evidence</h3>${(result.citations || []).map((citation) => `<div class="evidence-item">${escapeHtml(citation.summary)}<code>${escapeHtml(citation.source)} · ${escapeHtml(citation.locator)}</code></div>`).join("") || '<div class="empty-state">No citations returned.</div>'}</section>`;
}

const commands = [
  { label: "File: Save", key: "Ctrl+S", run: saveActiveFile },
  { label: "Build: Workspace ELF", key: "Ctrl+Shift+B", run: () => runVerification({ buildOnly: true }) },
  { label: "View: Explorer", key: "Ctrl+1", run: () => setPage("editor") },
  { label: "View: Verification", key: "Ctrl+2", run: () => setPage("verify") },
  { label: "View: Datasheet", key: "Ctrl+3", run: () => setPage("datasheet") },
  { label: "View: Simulation", key: "Ctrl+4", run: () => setPage("simulation") },
  { label: "View: Extensions & MCP", key: "Ctrl+5", run: () => setPage("extensions") },
  { label: "View: Getting started", key: "Ctrl+6", run: () => setPage("guide") },
  { label: "View: Split editor", key: "", run: () => { state.splitEditor = !state.splitEditor; syncSplitPane(); } },
  { label: "View: Toggle Panel", key: "Ctrl+J", run: () => togglePanel() },
  { label: "Run: Verification Loop", key: "Ctrl+Enter", run: runVerification },
  { label: "MCP: Refresh Servers", key: "", run: loadMcp },
];

function openPalette() {
  $("commandPalette").hidden = false;
  $("commandInput").value = "";
  renderCommands();
  requestAnimationFrame(() => $("commandInput").focus());
}

function closePalette() { $("commandPalette").hidden = true; }

function renderCommands() {
  const query = $("commandInput").value.trim().toLowerCase();
  const entries = [
    ...commands,
    ...state.workspace.map((file) => ({ label: `File: ${file.path}`, key: fileGlyph(file.path), run: () => openFile(file.path) })),
  ];
  const filtered = entries.filter((entry) => entry.label.toLowerCase().includes(query));
  $("commandList").innerHTML = filtered.map((entry, index) => `<button class="command-item ${index === 0 ? "active" : ""}" data-palette-index="${index}"><span>${escapeHtml(entry.label)}</span>${entry.key ? `<kbd>${escapeHtml(entry.key)}</kbd>` : ""}</button>`).join("");
  $$('[data-palette-index]', $("commandList")).forEach((button) => button.addEventListener("click", () => runPaletteEntry(filtered[Number(button.dataset.paletteIndex)])));
}

function runPaletteEntry(entry) {
  if (!entry) return;
  closePalette();
  entry.run();
}

function wireEvents() {
  $$(".activity-item").forEach((button) => button.addEventListener("click", () => {
    if (button.dataset.page === "editor" && !state.sidebarOpen) toggleSidebar(true);
    setPage(button.dataset.page);
  }));
  $$('[data-page-target]').forEach((button) => button.addEventListener("click", () => setPage(button.dataset.pageTarget)));
  $$('[data-command="palette"]').forEach((button) => button.addEventListener("click", openPalette));
  $$('[data-command="verify"]').forEach((button) => button.addEventListener("click", () => runVerification()));
  $$(".panel-tab").forEach((button) => button.addEventListener("click", () => setConsole(button.dataset.console)));
  $("toolbarSave").addEventListener("click", saveActiveFile);
  $("commandTrigger").addEventListener("click", openPalette);
  $("toolbarMcp").addEventListener("click", () => setPage("extensions"));
  $("toolbarVerify").addEventListener("click", () => runVerification());
  $("toolbarRun").addEventListener("click", () => runVerification());
  $("toolbarGenerate").addEventListener("click", () => generateFirmware());
  $("studioGenerate")?.addEventListener("click", () => generateFirmware());
  $("studioBuild")?.addEventListener("click", () => runVerification());
  $("intentInput")?.addEventListener("keydown", (event) => {
    if ((event.ctrlKey || event.metaKey) && event.key === "Enter") { event.preventDefault(); event.stopPropagation(); generateFirmware(); }
  });
  $("toolbarBuild").addEventListener("click", () => runVerification({ buildOnly: true }));
  $("pageRun")?.addEventListener("click", () => runVerification());
  $("pageSimulate")?.addEventListener("click", () => runVerification({ openSimulation: true }));
  $("refreshMcp").addEventListener("click", loadMcp);
  $("extensionSearch").addEventListener("input", (event) => renderMcp(event.target.value));
  $("clearConsole").addEventListener("click", () => {
    const visible = $("bottomPanel").querySelector('.panel-output:not([hidden])');
    if (visible) visible.innerHTML = "";
  });
  $("togglePanel").addEventListener("click", () => togglePanel(false));
  $("togglePanelTop").addEventListener("click", () => togglePanel());
  $("toggleSidebar").addEventListener("click", () => toggleSidebar());
  $("toggleAux").addEventListener("click", () => toggleAux());
  $("toggleSidebarTop").addEventListener("click", () => toggleSidebar());
  $("toggleAuxTop").addEventListener("click", () => toggleAux());
  $("splitEditor").addEventListener("click", () => { state.splitEditor = !state.splitEditor; syncSplitPane(); });
  $("editorMore").addEventListener("click", openPalette);
  $("sbProblems").addEventListener("click", () => { setPage("verify"); setConsole("problems"); togglePanel(true); });
  $("guideBuild").addEventListener("click", () => runVerification({ buildOnly: true }));
  $$('[data-guide-build]').forEach((button) => button.addEventListener("click", () => runVerification({ buildOnly: true })));
  $$('[data-guide-run]').forEach((button) => button.addEventListener("click", () => runVerification()));
  $$('[data-guide-page]').forEach((button) => button.addEventListener("click", () => setPage(button.dataset.guidePage)));
  $("codeEditor").addEventListener("input", onEditorInput);
  $("codeEditor").addEventListener("scroll", syncEditorScroll);
  $("codeEditor").addEventListener("click", renderCodeSurface);
  $("codeEditor").addEventListener("keyup", updateCursor);
  $("commandInput").addEventListener("input", renderCommands);
  $("commandInput").addEventListener("keydown", (event) => {
    if (event.key === "Enter") {
      event.preventDefault();
      $("commandList").querySelector("[data-palette-index]")?.click();
    }
  });
  $("commandPalette").addEventListener("click", (event) => { if (event.target === $("commandPalette")) closePalette(); });

  document.addEventListener("keydown", (event) => {
    const modifier = event.ctrlKey || event.metaKey;
    if (event.key === "Escape") closePalette();
    if (modifier && event.shiftKey && event.key.toLowerCase() === "p") { event.preventDefault(); openPalette(); }
    if (modifier && event.key.toLowerCase() === "s") { event.preventDefault(); saveActiveFile(); }
    if (modifier && event.key === "Enter") { event.preventDefault(); runVerification(); }
    if (modifier && event.shiftKey && event.key.toLowerCase() === "b") { event.preventDefault(); runVerification({ buildOnly: true }); }
    if (modifier && event.key.toLowerCase() === "j") { event.preventDefault(); togglePanel(); }
    if (modifier && /^[1-6]$/.test(event.key)) {
      event.preventDefault();
      setPage(["verify", "editor", "datasheet", "simulation", "extensions", "guide"][Number(event.key) - 1]);
    }
  });
}

async function boot() {
  wireEvents();
  syncSplitPane();
  setPage("verify");
  fillChips("simVerified", []);
  fillChips("simStubbed", []);
  fillChips("simOos", []);
  log("conBuild", "Starting Embeder workbench", "info", "core");

  const tasks = [
    transport.call("project").then(renderProject),
    transport.call("workspace_files").then((workspace) => { state.workspace = workspace.files || []; renderSidebar(); }),
    transport.call("peripherals").then(renderPeripherals),
  ];
  const results = await Promise.allSettled(tasks);
  results.filter((result) => result.status === "rejected").forEach((result) => reportProblem(result.reason?.message || "Startup integration failed"));
  await openFile("src/main.c");   // preload main.c so the Files view is ready to review
  setPage("verify");              // land on Build, not the editor
  await loadMcp();
  log("conBuild", "Workbench ready", "ok", "core");
}

boot();
