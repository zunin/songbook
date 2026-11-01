import init, { SongbookSession } from "./songbook_wasm.js";

const ACCEPTED_EXTENSIONS = [".txt", ".crd"];

const state = {
  session: null,
  snapshot: null,
  fileName: "",
  editing: false,
};

const els = {};

function $(id) {
  return document.getElementById(id);
}

function initEls() {
  for (const id of [
    "load-screen", "metadata-screen", "triage-screen", "preview-screen",
    "drop-zone", "file-input", "file-error",
    "metadata-form", "metadata-continue", "metadata-error",
    "triage-status", "triage-live", "triage-list", "start-over",
    "preview-content", "preview-back", "standalone-toggle", "download-tex",
  ]) {
    els[id] = $(id);
  }
}

function showScreen(name) {
  for (const id of ["load-screen", "metadata-screen", "triage-screen", "preview-screen"]) {
    const visible = id === name;
    els[id].hidden = !visible;
  }
  const titles = {
    "load-screen": "Songbook — Open a song file",
    "metadata-screen": "Songbook — Metadata",
    "triage-screen": "Songbook — Triage",
    "preview-screen": "Songbook — Preview",
  };
  document.title = titles[name] ?? "Songbook";
}

function announce(text) {
  els["triage-live"].textContent = text;
}

function statusText(snapshot) {
  const pending = snapshot.lines.filter((l) => l.status === "pending").length;
  return `${snapshot.lineCount} lines · ${snapshot.flagCount} flagged · ` +
    `${pending} pending · line ${snapshot.position + 1} current`;
}

function verdictLabel(line) {
  switch (line.verdict) {
    case "keep": return "kept";
    case "discard": return "discarded";
    case "lyric": return "lyric";
    case "chord_only": return "chord only";
    case "section_header": return "header";
    case "noise": return "noise";
    default: return "pending";
  }
}

function markerFor(line) {
  switch (line.status) {
    case "pending": return line.flag ? "?" : "·";
    case "dropped": return "×";
    case "preamble": return "·";
    case "edited": return "✎";
    default: return "✓";
  }
}

function renderMetadata(snapshot) {
  const form = els["metadata-form"];
  form.textContent = "";
  for (const field of snapshot.metadata.fields) {
    const wrap = document.createElement("div");
    wrap.className = "metadata-field";

    const inputId = `meta-field-${field.index}`;
    const label = document.createElement("label");
    label.setAttribute("for", inputId);
    label.textContent = field.label;

    const input = document.createElement("input");
    input.type = "text";
    input.id = inputId;
    input.value = field.value;
    input.autocomplete = "off";
    input.dataset.index = String(field.index);

    label.append(input);
    if (field.inferred) {
      const hint = document.createElement("span");
      hint.className = "hint";
      hint.textContent = "(inferred)";
      wrap.append(hint);
    }
    wrap.append(label);
    form.append(wrap);

    input.addEventListener("focus", () => syncMetadataField(field.index));
    input.addEventListener("input", () => {
      syncMetadataField(field.index);
      sessionSetField(input.value);
    });
    input.addEventListener("keydown", (event) => {
      if (event.key === "Enter") {
        event.preventDefault();
        els["metadata-continue"].focus();
      }
    });
  }
  const first = form.querySelector("input");
  if (first) first.focus();
}

function syncMetadataField(targetIndex) {
  const snapshot = state.snapshot;
  if (!snapshot || snapshot.phase !== "metadata") return;
  let guard = 0;
  while (state.snapshot.metadataField !== targetIndex && guard < 16) {
    const current = state.snapshot.metadataField;
    const count = state.snapshot.metadata.fields.length;
    const forward = (targetIndex - current + count) % count;
    const backward = (current - targetIndex + count) % count;
    applyAction(forward <= backward ? "tab" : "back_tab");
    guard += 1;
  }
}

function sessionSetField(value) {
  const session = state.session;
  const snapshot = state.snapshot;
  if (!session || !snapshot || snapshot.phase !== "metadata") return;
  session.handleAction("clear_field");
  if (value.length > 0) {
    session.insertText(value);
  }
  refresh();
}

function applyAction(action) {
  const session = state.session;
  if (!session) return;
  try {
    state.snapshot = session.handleAction(action);
  } catch (error) {
    announce(`Action failed: ${error}`);
    return;
  }
  render();
}

function render() {
  const snapshot = state.snapshot;
  if (!snapshot) return;
  switch (snapshot.phase) {
    case "metadata": renderMetadataPhase(snapshot); break;
    case "triage": renderTriage(snapshot); break;
    case "preview": renderPreviewPhase(snapshot); break;
    default: break;
  }
}

function renderMetadataPhase(snapshot) {
  showScreen("metadata-screen");
  if (!els["metadata-form"].childElementCount) {
    renderMetadata(snapshot);
  }
  refreshMetadataValues(snapshot);
}

function refreshMetadataValues(snapshot) {
  const inputs = els["metadata-form"].querySelectorAll("input");
  for (const input of inputs) {
    const index = Number(input.dataset.index);
    const field = snapshot.metadata.fields[index];
    if (field && document.activeElement !== input && input.value !== field.value) {
      input.value = field.value;
    }
  }
}

function renderTriage(snapshot) {
  showScreen("triage-screen");
  state.editing = snapshot.editing;
  els["triage-status"].textContent = statusText(snapshot);
  renderTriageList(snapshot);
}

function renderTriageList(snapshot) {
  const list = els["triage-list"];

  list.textContent = "";
  for (const line of snapshot.lines) {
    const item = document.createElement("div");
    item.setAttribute("role", "listitem");
    item.dataset.index = String(line.index);
    item.className = "triage-line";
    if (line.index === snapshot.position) item.classList.add("current");
    if (line.status === "pending") item.classList.add("pending");
    if (line.flag) item.classList.add("flagged");
    if (line.status === "preamble") item.classList.add("preamble");
    if (line.status === "edited") item.classList.add("edited");

    const marker = document.createElement("span");
    marker.className = "line-marker";
    marker.textContent = markerFor(line);
    marker.setAttribute("aria-hidden", "true");

    const number = document.createElement("span");
    number.className = "line-number";
    number.textContent = String(line.index + 1);
    number.setAttribute("aria-hidden", "true");

    const kind = document.createElement("span");
    kind.className = "line-kind";
    kind.textContent = line.classifier;

    const described = `status ${line.status}, verdict ${verdictLabel(line)}` +
      `${line.flag ? ", flagged" : ""}${line.index === snapshot.position ? ", current" : ""}`;

    if (state.editing && line.index === snapshot.position) {
      item.append(marker, number, kind, buildEditInput(line));
    } else {
      const select = document.createElement("button");
      select.type = "button";
      select.className = "line-select";
      select.setAttribute("aria-label",
        `Line ${line.index + 1}, ${line.classifier}, ${described}: ` +
        (line.edited ? line.text : line.original));
      const text = document.createElement("span");
      text.className = "line-text";
      text.textContent = line.edited ? line.text : line.original;
      select.append(text);
      select.addEventListener("click", () => moveToLine(line.index));
      item.append(marker, number, kind, select);
    }

    list.append(item);
  }

  const current = list.querySelector(".current");
  if (current) {
    const target = current.querySelector(".line-edit-input") ??
      current.querySelector(".line-select");
    if (target && document.activeElement !== target) target.focus();
  }
  els["triage-status"].textContent = statusText(snapshot);
}

function buildEditInput(line) {
  const input = document.createElement("input");
  input.type = "text";
  input.className = "line-edit-input";
  input.value = state.snapshot.editBuffer ?? state.snapshot.lines[line.index].text;
  input.setAttribute("aria-label",
    `Editing line ${line.index + 1}: ${input.value}`);
  input.dataset.previous = input.value;

  input.addEventListener("input", () => {
    const session = state.session;
    const previous = input.dataset.previous ?? "";
    const value = input.value;
    if (value.startsWith(previous)) {
      const added = value.slice(previous.length);
      if (added.length > 0) session.insertText(added);
    } else if (previous.startsWith(value)) {
      for (let i = 0; i < previous.length - value.length; i += 1) {
        session.handleAction("backspace");
      }
    } else {
      for (let i = 0; i < previous.length; i += 1) {
        session.handleAction("backspace");
      }
      if (value.length > 0) session.insertText(value);
    }
    input.dataset.previous = value;
    state.snapshot = state.session.snapshot();
  });

  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter") {
      event.preventDefault();
      applyAction("confirm");
    } else if (event.key === "Escape") {
      event.preventDefault();
      applyAction("escape");
    } else if (event.key === "Tab") {
      event.preventDefault();
      applyAction("confirm");
    }
  });

  return input;
}

function moveToLine(targetIndex) {
  const session = state.session;
  const snapshot = state.snapshot;
  if (!session || !snapshot) return;
  let guard = 0;
  const last = snapshot.lineCount - 1;
  while (state.snapshot.position !== targetIndex && guard <= last) {
    const forward = targetIndex > state.snapshot.position;
    try {
      session.handleAction(forward ? "next" : "prev");
    } catch (error) {
      break;
    }
    guard += 1;
  }
  state.snapshot = session.snapshot();
  render();
  const row = els["triage-list"].querySelector(".current .line-select");
  if (row) row.focus();
}

function renderPreviewPhase(snapshot) {
  const container = els["preview-content"];
  const previewHadFocus = container.contains(document.activeElement);
  showScreen("preview-screen");
  container.textContent = "";
  for (const row of sessionPreviewRows()) {
    const div = document.createElement("div");
    div.className = `row-${row.style}`;
    div.textContent = row.text === "" ? " " : row.text;
    container.append(div);
  }
  container.scrollTop = Array.from(container.children)
    .slice(0, snapshot.previewScroll)
    .reduce((top, row) => top + row.offsetHeight, 0);
  if (!previewHadFocus) container.focus();
  announce("Preview updated.");
}

function sessionPreviewRows() {
  try {
    return state.session.previewRows();
  } catch (error) {
    announce(`Preview failed: ${error}`);
    return [];
  }
}

function refresh() {
  state.snapshot = state.session.snapshot();
  render();
}

function downloadTex() {
  const session = state.session;
  if (!session) return;
  const standalone = els["standalone-toggle"].checked;
  try {
    const tex = session.exportTex(standalone);
    const stem = state.fileName.replace(/\.[^.]+$/, "") || "song";
    const blob = new Blob([tex], { type: "text/x-tex" });
    const url = URL.createObjectURL(blob);
    const link = document.createElement("a");
    link.href = url;
    link.download = `${stem}.tex`;
    document.body.append(link);
    link.click();
    link.remove();
    window.setTimeout(() => URL.revokeObjectURL(url), 1000);
    announce(`Downloaded ${stem}.tex.`);
  } catch (error) {
    announce(`Export failed: ${error}`);
  }
}

function startOver() {
  const confirmed = window.confirm(
    "Start over? All triage decisions for this song will be lost.");
  if (!confirmed) return;
  state.session = null;
  state.snapshot = null;
  state.fileName = "";
  state.editing = false;
  els["metadata-form"].textContent = "";
  els["file-input"].value = "";
  els["file-error"].textContent = "";
  showScreen("load-screen");
  els["file-input"].focus();
}

function loadFile(file) {
  const extension = file.name.slice(file.name.lastIndexOf(".")).toLowerCase();
  if (!ACCEPTED_EXTENSIONS.includes(extension)) {
    els["file-error"].textContent =
      `Unsupported file type “${file.name}”. Please choose a .txt or .crd file.`;
    return;
  }
  file.text().then((raw) => {
    try {
      state.session = new SongbookSession(raw);
      state.fileName = file.name;
      els["file-error"].textContent = "";
      els["metadata-form"].textContent = "";
      refresh();
    } catch (error) {
      els["file-error"].textContent = String(error);
    }
  }).catch((error) => {
    els["file-error"].textContent = `Could not read ${file.name}: ${error}`;
  });
}

function setupGlobalKeys() {
  document.addEventListener("keydown", (event) => {
    const target = event.target;
    if (target instanceof HTMLInputElement ||
        target instanceof HTMLTextAreaElement ||
        target instanceof HTMLSelectElement) {
      return;
    }
    if (event.ctrlKey || event.metaKey || event.altKey) {
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "z" &&
          state.snapshot?.phase === "triage") {
        event.preventDefault();
        applyAction("undo");
      }
      return;
    }
    if (state.snapshot?.phase !== "triage" &&
        state.snapshot?.phase !== "preview") {
      return;
    }
    const key = event.key;
    let handled = true;
    if (key === "ArrowDown" || key === "j") applyAction("next");
    else if (key === "ArrowUp" || key === "k") applyAction("prev");
    else if (key === "PageDown") applyAction("page_down");
    else if (key === "PageUp") applyAction("page_up");
    else if (key === "f") applyAction("next_flag");
    else if (key === "F") applyAction("prev_flag");
    else if (key === "Enter") applyAction("keep");
    else if (key === "d") applyAction("discard");
    else if (key === "l") applyAction("lyric");
    else if (key === "c") applyAction("chord_only");
    else if (key === "h") applyAction("section_header");
    else if (key === "n") applyAction("noise");
    else if (key === "e") applyAction("edit");
    else if (key === "u") applyAction("undo");
    else if (key === "a") applyAction("auto_accept");
    else if (key === "p" || key === "s") applyAction("preview");
    else if (key === "Escape") applyAction("escape");
    else handled = false;

    if (handled) {
      event.preventDefault();
      const snapshot = state.snapshot;
      if (snapshot?.message) announce(snapshot.message);
    }
  });
}

function setupControls() {
  const zone = els["drop-zone"];

  els["file-input"].addEventListener("change", () => {
    const file = els["file-input"].files[0];
    if (file) loadFile(file);
  });

  for (const type of ["dragenter", "dragover"]) {
    zone.addEventListener(type, (event) => {
      event.preventDefault();
      zone.classList.add("dragover");
    });
  }
  for (const type of ["dragleave", "drop"]) {
    zone.addEventListener(type, () => zone.classList.remove("dragover"));
  }
  zone.addEventListener("drop", (event) => {
    event.preventDefault();
    const file = event.dataTransfer?.files?.[0];
    if (file) loadFile(file);
  });

  els["metadata-continue"].addEventListener("click", () => applyAction("confirm"));
  els["start-over"].addEventListener("click", startOver);

  for (const button of els["triage-screen"].querySelectorAll("button[data-action]")) {
    button.addEventListener("click", () => {
      applyAction(button.dataset.action);
      const snapshot = state.snapshot;
      if (snapshot?.message) announce(snapshot.message);
    });
  }

  els["preview-back"].addEventListener("click", () => applyAction("back"));
  els["download-tex"].addEventListener("click", downloadTex);
  els["standalone-toggle"].addEventListener("change", () => announce(
    els["standalone-toggle"].checked
      ? "Standalone document enabled."
      : "Package song format selected."));
}

async function boot() {
  initEls();
  setupControls();
  setupGlobalKeys();
  showScreen("load-screen");
  try {
    await init();
  } catch (error) {
    els["file-error"].textContent =
      "Failed to load the song engine (WebAssembly). " +
      "Make sure you serve this folder over HTTP(S).";
    return;
  }
  els["file-input"].focus();
}

boot();
