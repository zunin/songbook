import init, { SongbookSession, composeBook } from "./songbook_wasm.js";

const ACCEPTED_EXTENSIONS = [".txt", ".crd"];

const state = {
  session: null,
  snapshot: null,
  fileName: "",
  editing: false,
  pdfEngine: null,
  pdfBusy: false,
  screen: "load-screen",
  // Book mode (ADR0019): null in the single-song flow, otherwise
  // { results: [source, …], current, title, author, chordIndex, nav } —
  // memory only (P3). A source (ADR0020) is either a chosen file
  // ({ kind: "file", file, name, … }) or pasted tab text held in `text`
  // ({ kind: "paste", text, name, … }).
  book: null,
};

const els = {};

function $(id) {
  return document.getElementById(id);
}

function initEls() {
  for (const id of [
    "load-screen", "metadata-screen", "triage-screen", "preview-screen",
    "drop-zone", "file-input", "file-error",
    "paste-input", "paste-use", "paste-error",
    "book-paste-input", "book-paste-add", "book-paste-error",
    "export-paste-input", "export-paste-add", "export-paste-error",
    "metadata-form", "metadata-continue", "metadata-error",
    "triage-status", "triage-live", "triage-list", "start-over",
    "preview-content", "preview-back", "standalone-toggle", "download-tex",
    "download-pdf", "pdf-status",
    "transpose-down", "transpose-up", "transpose-reset", "transpose-label",
    "book-banner", "book-progress", "book-file-error",
    "book-form-screen", "book-form-status", "book-title-input",
    "book-author-input", "book-form-continue", "book-form-reset",
    "metadata-back", "triage-back", "book-next",
    "book-export-screen", "book-summary", "book-export-failed",
    "chord-index-toggle", "book-export-tex", "book-export-pdf",
    "book-export-status", "book-pdf-status", "book-export-back",
    "book-export-reset",
  ]) {
    els[id] = $(id);
  }
}

function showScreen(name) {
  state.screen = name;
  for (const id of ["load-screen", "book-form-screen", "metadata-screen",
      "triage-screen", "preview-screen", "book-export-screen"]) {
    const visible = id === name;
    els[id].hidden = !visible;
  }
  const titles = {
    "load-screen": "Songbook — Open a song file",
    "book-form-screen": "Songbook — Book details",
    "metadata-screen": "Songbook — Metadata",
    "triage-screen": "Songbook — Triage",
    "preview-screen": "Songbook — Preview",
    "book-export-screen": "Songbook — Book export",
  };
  document.title = titles[name] ?? "Songbook";
  const onSong = name === "metadata-screen" || name === "triage-screen" ||
    name === "preview-screen";
  els["book-banner"].hidden = !(state.book && onSong);
}

function announce(text) {
  els["triage-live"].textContent = text;
}

function statusText(snapshot) {
  const pending = snapshot.lines.filter((l) => l.status === "pending").length;
  return `${snapshot.lineCount} lines · ${snapshot.flagCount} flagged · ` +
    `${pending} pending · line ${snapshot.position + 1} current`;
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

// The label slot names what the line IS NOW (the snapshot's effectiveLabel);
// the parser's name only survives as a muted "(was: …)" suffix when the two
// differ. One lowercase vocabulary: lyric, chord-only, section header,
// fretboard, noise, dropped, discarded, ambiguous, blank.
function wasSuffix(line) {
  return line.effectiveLabel === line.classifier
    ? ""
    : ` (was: ${line.classifier})`;
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

// Bulk accept ("a" / "Accept remaining pending"): pending lines become keeps
// and the session moves to the preview. The summary counts the lines that
// were accepted and how many of those were plain parser guesses (ambiguous
// flags resolved to lyrics), from the pre-action snapshot.
function applyAutoAccept() {
  const before = state.snapshot;
  const accepted = before
    ? before.lines.filter((line) => line.status === "pending")
    : [];
  applyAction("auto_accept");
  if (!state.snapshot || state.snapshot === before) return; // failed: reported
  const guesses = accepted.filter((line) => line.classifier === "ambiguous").length;
  announce(`Accepted ${accepted.length} pending lines, ${guesses} were parser guesses.`);
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
  els["metadata-back"].hidden = !state.book;
  if (state.book) {
    els["metadata-back"].textContent = bookBackLabel();
  }
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
  els["triage-back"].hidden = !state.book;
  if (state.book) {
    els["triage-back"].textContent = bookBackLabel();
  }
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
    kind.textContent = line.effectiveLabel;
    if (line.effectiveLabel !== line.classifier) {
      const was = document.createElement("span");
      was.className = "was";
      was.textContent = wasSuffix(line);
      kind.append(was);
    }

    const described = `status ${line.status}` +
      `${line.flag ? ", flagged" : ""}${line.index === snapshot.position ? ", current" : ""}`;

    if (state.editing && line.index === snapshot.position) {
      item.append(marker, number, kind, buildEditInput(line));
    } else {
      const select = document.createElement("button");
      select.type = "button";
      select.className = "line-select";
      select.setAttribute("aria-label",
        `Line ${line.index + 1}, ${line.effectiveLabel}${wasSuffix(line)}, ${described}: ` +
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
  // Clicking a transpose control re-renders the preview; keep focus on the
  // button instead of yanking it into the scroll container.
  const previewHadFocus = container.contains(document.activeElement) ||
    TRANSPOSE_IDS.some((id) => els[id] === document.activeElement);
  showScreen("preview-screen");
  const book = Boolean(state.book);
  els["standalone-toggle"].closest("label").hidden = book;
  els["download-tex"].hidden = book;
  els["download-pdf"].hidden = book;
  els["pdf-status"].hidden = book;
  els["book-next"].hidden = !book;
  if (state.book) {
    els["book-next"].textContent = bookForwardLabel();
  }
  renderTransposeLabel(snapshot);
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

// --- Transposition (roadmap item 3): the session keeps the net offset, so
// +1 then −1 is a net zero, and every derived view (preview rows, the .tex
// and PDF downloads, and in book mode each song's book part) follows it.

const TRANSPOSE_IDS = ["transpose-down", "transpose-up", "transpose-reset"];

function renderTransposeLabel(snapshot) {
  const label = els["transpose-label"];
  const offset = snapshot.transpose ?? 0;
  if (offset === 0) {
    label.hidden = true;
    label.textContent = "";
    return;
  }
  label.hidden = false;
  label.textContent = `Transposed ${offset > 0 ? "+" : "−"}${Math.abs(offset)}`;
}

function applyTranspose(delta) {
  const session = state.session;
  if (!session) return;
  try {
    state.snapshot = session.transpose(delta);
  } catch (error) {
    announce(`Transpose failed: ${error}`);
    return;
  }
  render();
  announceTranspose(delta);
}

// Back to the song's original key, shared by the Reset button and the `0`
// key (preview phase). Judged on the session offset the snapshot reports —
// the same surface the label and the announcements read.
function resetTranspose() {
  const offset = state.snapshot?.transpose ?? 0;
  if (offset !== 0) applyTranspose(-offset);
  else announce("Transposition is already at the original key.");
}

function announceTranspose(delta) {
  const offset = state.snapshot?.transpose ?? 0;
  if (offset === 0) {
    announce("Transposition reset to the original key.");
    return;
  }
  const step = Math.abs(delta) === 1 ? "one semitone" : `${Math.abs(delta)} semitones`;
  const direction = delta > 0 ? "up" : "down";
  const first = firstPreviewChord();
  announce(`Transposed ${direction} ${step} — now at ${
    offset > 0 ? "+" : "−"}${Math.abs(offset)}` +
    (first ? `, first chord ${first}.` : "."));
}

// The first chord of the refreshed preview, for the spoken announcement:
// chord-only rows hold bare tokens, lyric rows hold `[Chord]` markers.
function firstPreviewChord() {
  for (const row of sessionPreviewRows()) {
    if (row.style === "chord") {
      for (const token of row.text.trim().split(/\s+/)) {
        if (/^[A-G][#b]?/.test(token)) return token;
      }
    } else if (row.style === "lyric") {
      const match = row.text.match(/\[([A-G][#b]?[^\]]*)\]/);
      if (match) return match[1];
    }
  }
  return null;
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

// --- Book mode (ADR0019): several songs, triaged one at a time in the
// existing session screens, composed into one book by core's `composeBook`.
// Each entry is a source (ADR0020): either a chosen file or pasted tab text
// kept in `entry.text` (memory only, P3). `results` order is add order —
// the initially chosen files in selection order, then each pasted song in
// the order it was added — and that one array order is the triage order and
// the book part order alike. Failed sources are marked, reported, and
// excluded — never dropped silently.

function startBook(files) {
  state.book = {
    results: files.map((file) => (
      { kind: "file", file, text: null, name: file.name,
        session: null, error: null, loading: false })),
    current: -1,
    title: "",
    author: "",
    chordIndex: false,
    // Navigation epoch: in-flight file loads must not yank the user after
    // they have moved to another book screen.
    nav: 0,
  };
  showBookForm();
}

// "2 files + 1 pasted song" — the shape of the book's sources.
function bookSourceSummary(book) {
  const files = book.results.filter((e) => e.kind === "file").length;
  const pastes = book.results.filter((e) => e.kind === "paste").length;
  const parts = [];
  if (files > 0) parts.push(`${files} ${files === 1 ? "file" : "files"}`);
  if (pastes > 0) {
    parts.push(`${pastes} ${pastes === 1 ? "pasted song" : "pasted songs"}`);
  }
  return parts.join(" + ");
}

function bookFormStatusText(book) {
  return `${bookSourceSummary(book)} added. Songs are triaged one at a ` +
    "time and appear in the book in the order the sources were added.";
}

// Display label for the next pasted entry: "Pasted song", then
// "Pasted song 2", "Pasted song 3", … so notices can tell them apart.
function nextPasteName(book) {
  const pastes = book.results.filter((e) => e.kind === "paste").length;
  return pastes === 0 ? "Pasted song" : `Pasted song ${pastes + 1}`;
}

// Adds the textarea's contents as a new pasted source at the end of the
// book (ADR0020). Only the raw text is stored; a session is created when
// triage reaches the entry, exactly as for a file — so text the engine
// cannot use fails there, with the same per-entry report as a failed file.
function addPastedSong(screen) {
  const book = state.book;
  if (!book) return;
  const input = els[screen === "form" ? "book-paste-input" : "export-paste-input"];
  const error = els[screen === "form" ? "book-paste-error" : "export-paste-error"];
  const raw = input.value;
  if (raw.trim().length === 0) {
    error.textContent = "Paste some tab text first.";
    return;
  }
  book.results.push({
    kind: "paste", file: null, text: raw, name: nextPasteName(book),
    session: null, error: null, loading: false,
  });
  input.value = "";
  error.textContent = "";
  if (screen === "form") {
    els["book-form-status"].textContent = bookFormStatusText(book);
    input.focus();
  } else {
    bookForward(); // triage the appended song right away
  }
}

function showBookForm() {
  const book = state.book;
  if (!book) return;
  book.nav += 1;
  // The form sits "before" every song: going back to it and pressing
  // Start triage must resume at the first song, not skip it.
  book.current = -1;
  els["book-title-input"].value = book.title;
  els["book-author-input"].value = book.author;
  showScreen("book-form-screen");
  els["book-form-status"].textContent = bookFormStatusText(book);
  els["book-title-input"].focus();
}

function startBookTriage() {
  const book = state.book;
  if (!book) return;
  // Raw strings: composeBook does the LaTeX escaping in core.
  book.title = els["book-title-input"].value;
  book.author = els["book-author-input"].value;
  bookForward();
}

function bookForward() {
  const book = state.book;
  if (!book) return;
  const here = book.results[book.current];
  if (here && here.loading) return; // wait for the in-flight load
  for (let i = book.current + 1; i < book.results.length; i += 1) {
    const entry = book.results[i];
    if (entry.error) continue;  // failed sources stay excluded
    if (entry.loading) return;
    book.nav += 1;
    book.current = i;
    if (entry.session) showSong(i);
    else loadBookSong(i);
    return;
  }
  book.nav += 1;
  showBookExport();
}

function bookBack() {
  const book = state.book;
  if (!book) return;
  for (let i = book.current - 1; i >= 0; i -= 1) {
    if (book.results[i].session) {
      book.nav += 1;
      book.current = i;
      showSong(i);
      return;
    }
  }
  showBookForm();
}

function bookExportBack() {
  const book = state.book;
  if (!book) return;
  for (let i = book.results.length - 1; i >= 0; i -= 1) {
    if (book.results[i].session) {
      book.nav += 1;
      book.current = i;
      showSong(i);
      return;
    }
  }
  showBookForm();
}

// Shows the song at `index` in whatever phase its session is in (metadata,
// triage, or preview), so revisits resume exactly where they left off.
function showSong(index) {
  const book = state.book;
  const entry = book.results[index];
  state.session = entry.session;
  state.fileName = entry.name;
  els["metadata-form"].textContent = "";
  refresh();
  els["book-progress"].textContent =
    `Song ${index + 1} of ${book.results.length} — ${entry.name}`;
  document.title = `Songbook — Song ${index + 1} of ${book.results.length}`;
}

function loadBookSong(index) {
  const book = state.book;
  const entry = book.results[index];
  const nav = book.nav;
  entry.loading = true;
  const fail = (message) => {
    entry.loading = false;
    if (entry.error) return;
    entry.error = message;
    const notice = els["book-file-error"];
    notice.hidden = false;
    notice.textContent = `Could not use “${entry.name}”: ${message} ` +
      "It was skipped and will not appear in the book.";
    if (state.book === book && book.current === index && book.nav === nav) {
      bookForward();
    }
  };
  if (entry.kind === "paste") {
    // Pasted text (ADR0020) is already in memory: the same Session-creation
    // path as file contents, without the file read or extension check.
    entry.loading = false;
    try {
      entry.session = new SongbookSession(entry.text);
    } catch (error) {
      fail(String(error));
      return;
    }
    if (book.current === index && book.nav === nav) showSong(index);
    return;
  }
  const extension = entry.name.slice(entry.name.lastIndexOf(".")).toLowerCase();
  if (!ACCEPTED_EXTENSIONS.includes(extension)) {
    fail("unsupported file type (please use .txt or .crd files)");
    return;
  }
  entry.file.text().then((raw) => {
    if (state.book !== book) return; // the book was started over
    entry.loading = false;
    if (entry.error || entry.session) return; // another load already decided
    try {
      entry.session = new SongbookSession(raw);
    } catch (error) {
      fail(String(error));
      return;
    }
    if (book.current === index && book.nav === nav) showSong(index);
  }).catch((error) => {
    if (state.book !== book) return;
    fail(`could not be read (${error})`);
  });
}

function bookBackLabel() {
  const book = state.book;
  for (let i = book.current - 1; i >= 0; i -= 1) {
    if (book.results[i].session) return "Back to previous song";
  }
  return "Back to book details";
}

function bookForwardLabel() {
  const book = state.book;
  for (let i = book.current + 1; i < book.results.length; i += 1) {
    if (!book.results[i].error) return "Next song";
  }
  return "Continue to book export";
}

function showBookExport() {
  const book = state.book;
  if (!book) return;
  book.nav += 1;
  const done = book.results.filter((entry) => entry.session).length;
  const failed = book.results.filter((entry) => entry.error);
  showScreen("book-export-screen");
  els["book-summary"].textContent =
    `${done} ${done === 1 ? "song" : "songs"} ready for the book` +
    (failed.length ? ` · ${failed.length} skipped` : "") + ".";
  const failedBox = els["book-export-failed"];
  failedBox.textContent = "";
  for (const entry of failed) {
    const line = document.createElement("p");
    line.className = "error";
    line.textContent = `Skipped “${entry.name}”: ${entry.error}`;
    failedBox.append(line);
  }
  failedBox.hidden = failed.length === 0;
  els["chord-index-toggle"].checked = book.chordIndex;
  const exportable = done > 0;
  els["book-export-tex"].disabled = !exportable;
  els["book-export-pdf"].disabled = !exportable;
  els["book-export-back"].hidden = !exportable;
  bookStatus(exportable ? "" :
    "No songs loaded successfully, so there is nothing to export. " +
    "Add a pasted song below or press Start over to choose files again.");
  els["book-pdf-status"].textContent = "";
  els["book-pdf-status"].className = "";
  if (exportable) els["book-export-tex"].focus();
  else els["book-export-reset"].focus();
}

function buildTex() {
  const session = state.session;
  if (!session) return null;
  return session.exportTex(els["standalone-toggle"].checked);
}

function saveBlob(blob, filename) {
  const url = URL.createObjectURL(blob);
  const link = document.createElement("a");
  link.href = url;
  link.download = filename;
  document.body.append(link);
  link.click();
  link.remove();
  window.setTimeout(() => URL.revokeObjectURL(url), 1000);
}

// Download stem: a chosen file keeps its filename; a pasted song has none,
// so the stem derives from the song title once known — every snapshot
// carries the confirmed metadata — falling back to "song".
function downloadStem() {
  if (state.fileName) return state.fileName.replace(/\.[^.]+$/, "") || "song";
  const title = state.snapshot?.metadata.title ?? "";
  const stem = title.trim()
    .replace(/[^\p{L}\p{N}]+/gu, "-").replace(/^-+|-+$/g, "");
  return stem || "song";
}

function downloadTex() {
  try {
    const tex = buildTex();
    if (tex === null) return;
    const stem = downloadStem();
    saveBlob(new Blob([tex], { type: "text/x-tex" }), `${stem}.tex`);
    announce(`Downloaded ${stem}.tex.`);
  } catch (error) {
    announce(`Export failed: ${error}`);
  }
}

function pdfStatus(text, kind) {
  els["pdf-status"].textContent = text;
  els["pdf-status"].className = kind;
  announce(text);
}

function pdfProblems(result) {
  const problems = [];
  if (result.status !== "spotless") {
    problems.push(`compile status “${result.status}”`);
  }
  if (Array.isArray(result.missingFiles) && result.missingFiles.length > 0) {
    problems.push(`missing files: ${result.missingFiles.join(", ")}`);
  }
  if (Array.isArray(result.diagnostics) && result.diagnostics.length > 0) {
    const lines = result.diagnostics.map(
      (entry) => (typeof entry === "string" ? entry : JSON.stringify(entry)));
    problems.push(`diagnostics: ${lines.join(" | ")}`);
  }
  return problems.join("; ");
}

// Lazy PDF-engine bootstrap shared by the single-song and book exports.
// `status` is the pdfStatus-style reporter of the calling screen.
async function ensurePdfEngine(status) {
  if (!state.pdfEngine) {
    status("Loading PDF engine…", "hint");
    const pdf = await import("./pdf-assets/pdf-engine.js");
    state.pdfEngine = await pdf.createPdfEngine({ assetBase: "pdf-assets" });
    // Fetch the engine/assets under the loading label; compile() only
    // starts once the worker has actually reported ready.
    await state.pdfEngine.ready();
  }
  return state.pdfEngine;
}

async function downloadPdf() {
  if (state.pdfBusy) return;
  state.pdfBusy = true;
  els["download-pdf"].disabled = true;
  try {
    const tex = buildTex();
    if (tex === null) {
      pdfStatus("No song is loaded.", "error");
      return;
    }
    const stem = downloadStem();
    await ensurePdfEngine(pdfStatus);
    pdfStatus("Compiling PDF…", "hint");
    const result = await state.pdfEngine.compile(tex);
    const problems = pdfProblems(result);
    if (!result.bytes || !result.blob) {
      pdfStatus("PDF compilation produced no output" +
        (problems ? ` (${problems})` : "") + ".", "error");
      return;
    }
    saveBlob(result.blob, `${stem}.pdf`);
    pdfStatus(problems
      ? `Downloaded ${stem}.pdf with warnings: ${problems}.`
      : `Downloaded ${stem}.pdf.`,
      problems ? "warning" : "hint");
  } catch (error) {
    const poisoned = String(error).includes("poisoned compile");
    pdfStatus(poisoned
      ? "PDF compilation failed (the engine hit an unrecoverable state). " +
        "Press Download PDF to try again."
      : `PDF export failed: ${error}`,
      "error");
    if (!poisoned) {
      // Drop the wedged worker and forget the engine so a retry re-enters
      // the honest "Loading PDF engine…" path; the HTTP cache makes the
      // refetch cheap. A poisoned compile has already respawned its worker
      // and the engine object stays usable, so it is kept.
      await state.pdfEngine?.cancel();
      state.pdfEngine = null;
    }
  } finally {
    state.pdfBusy = false;
    els["download-pdf"].disabled = false;
  }
}

// --- Book export (ADR0019): composition happens only here, after every
// song's triage is complete. LaTeX is never assembled in JavaScript (P1/P2):
// each completed session hands over its `{ body, chords }` part and core's
// `composeBook` renders the standalone book document.

function bookStatus(text, kind = "") {
  els["book-export-status"].textContent = text;
  els["book-export-status"].className = kind;
}

function bookPdfStatus(text, kind) {
  els["book-pdf-status"].textContent = text;
  els["book-pdf-status"].className = kind;
}

function bookStem() {
  const stem = state.book.title.trim()
    .replace(/[^\p{L}\p{N}]+/gu, "-").replace(/^-+|-+$/g, "");
  return stem || "book";
}

function buildBookTex() {
  const book = state.book;
  const parts = [];
  for (const entry of book.results) {
    if (entry.session) parts.push(entry.session.bookParts());
  }
  return composeBook(
    book.title.trim() || null, book.author.trim() || null,
    parts, book.chordIndex);
}

function exportBookTex() {
  if (!state.book) return;
  try {
    const tex = buildBookTex();
    const stem = bookStem();
    saveBlob(new Blob([tex], { type: "text/x-tex" }), `${stem}.tex`);
    bookStatus(`Downloaded ${stem}.tex.`);
  } catch (error) {
    bookStatus(`Book export failed: ${error}`, "error");
  }
}

async function downloadBookPdf() {
  const book = state.book;
  if (!book || state.pdfBusy) return;
  state.pdfBusy = true;
  els["book-export-pdf"].disabled = true;
  try {
    const tex = buildBookTex();
    const stem = bookStem();
    const engine = await ensurePdfEngine(bookPdfStatus);
    bookPdfStatus("Compiling book PDF…", "hint");
    const result = await engine.compile(tex);
    const problems = pdfProblems(result);
    if (!result.bytes || !result.blob) {
      bookPdfStatus("PDF compilation produced no output" +
        (problems ? ` (${problems})` : "") + ".", "error");
      return;
    }
    saveBlob(result.blob, `${stem}.pdf`);
    bookPdfStatus(problems
      ? `Downloaded ${stem}.pdf with warnings: ${problems}.`
      : `Downloaded ${stem}.pdf.`,
      problems ? "warning" : "hint");
  } catch (error) {
    const poisoned = String(error).includes("poisoned compile");
    bookPdfStatus(poisoned
      ? "PDF compilation failed (the engine hit an unrecoverable state). " +
        "Press Download book PDF to try again."
      : `PDF export failed: ${error}`,
      "error");
    if (!poisoned) {
      await state.pdfEngine?.cancel();
      state.pdfEngine = null;
    }
  } finally {
    state.pdfBusy = false;
    els["book-export-pdf"].disabled = false;
  }
}

function startOver() {
  const confirmed = window.confirm(state.book
    ? "Start over? The whole book — every song's triage decisions — will be lost."
    : "Start over? All triage decisions for this song will be lost.");
  if (!confirmed) return;
  state.session = null;
  state.snapshot = null;
  state.fileName = "";
  state.editing = false;
  state.book = null;
  els["metadata-form"].textContent = "";
  els["file-input"].value = "";
  els["file-error"].textContent = "";
  els["paste-input"].value = "";
  els["paste-error"].textContent = "";
  els["book-paste-input"].value = "";
  els["book-paste-error"].textContent = "";
  els["export-paste-input"].value = "";
  els["export-paste-error"].textContent = "";
  els["book-file-error"].hidden = true;
  els["book-file-error"].textContent = "";
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

// --- Paste input (ADR0020): the load screen's textarea feeds the SAME
// session-creation path as a dropped file. No Clipboard API is used — the
// user pastes with the browser's own gesture — and the text lives only in
// the textarea and the session (memory, cleared by Start over).

function usePastedText() {
  const raw = els["paste-input"].value;
  if (raw.trim().length === 0) {
    els["paste-error"].textContent =
      "Paste some tab text first, or choose a file above.";
    return;
  }
  try {
    state.session = new SongbookSession(raw);
  } catch (error) {
    els["paste-error"].textContent = String(error);
    return;
  }
  state.fileName = ""; // no filename: download names derive from the title
  els["paste-error"].textContent = "";
  els["metadata-form"].textContent = "";
  refresh();
}

function setupGlobalKeys() {
  document.addEventListener("keydown", (event) => {
    const target = event.target;
    if (target instanceof HTMLInputElement ||
        target instanceof HTMLTextAreaElement ||
        target instanceof HTMLSelectElement) {
      return;
    }
    if (state.screen !== "triage-screen" &&
        state.screen !== "preview-screen") {
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
    // Transposition keys speak through applyTranspose's own announcement;
    // the shared tail below must not overwrite it with a stale
    // snapshot.message.
    let announced = false;
    if (key === "ArrowDown" || key === "j") applyAction("next");
    else if (key === "ArrowUp" || key === "k") applyAction("prev");
    else if (key === "PageDown") applyAction("page_down");
    else if (key === "PageUp") applyAction("page_up");
    else if (key === "f") applyAction("next_flag");
    else if (key === "F") applyAction("prev_flag");
    // Triage only. Core maps Key::Confirm to Phase::Done in the preview
    // phase (triage.rs handle_preview), a phase the web render() ignores, so
    // dispatching "keep" there would brick the session — and the
    // preventDefault below would also swallow a focused preview button's
    // native Enter click. On the preview screen Enter stays the browser's.
    else if (key === "Enter") {
      if (state.snapshot?.phase === "triage") applyAction("keep");
      else handled = false;
    }
    else if (key === "d") applyAction("discard");
    else if (key === "l") applyAction("lyric");
    else if (key === "c") applyAction("chord_only");
    else if (key === "h") applyAction("section_header");
    else if (key === "n") applyAction("noise");
    else if (key === "e") applyAction("edit");
    else if (key === "u") applyAction("undo");
    else if (key === "a") {
      // The summary is announced by applyAutoAccept; the shared tail below
      // must not overwrite it with a stale snapshot.message.
      applyAutoAccept();
      announced = true;
    }
    else if (key === "p" || key === "s") applyAction("preview");
    else if (key === "+" || key === "=" || key === "-" || key === "0") {
      // Preview phase only, mirroring the Transpose buttons. Deliberately
      // NOT handleAction("transpose_up"/"transpose_down"/"transpose_reset")
      // (the action names the TUI surface accepts): in the wasm Session
      // those drive the separate core TriageState offset, while the label,
      // the announcements and the buttons all read the session-level offset
      // that Session::transpose keeps — driving both would let them
      // accumulate independently and double-apply.
      if (state.snapshot?.phase === "preview") {
        if (key === "0") resetTranspose();
        else applyTranspose(key === "-" ? -1 : 1);
        announced = true;
      } else {
        handled = false;
      }
    }
    // Preview only. Core maps Key::Escape to back-to-triage there (the
    // preview-back button), but in the triage phase it maps to Phase::Quit
    // (triage.rs handle_triage), which the web render() ignores — the
    // session would brick. On the triage screen Escape is inert.
    else if (key === "Escape") {
      if (state.snapshot?.phase === "preview") applyAction("escape");
      else handled = false;
    }
    else handled = false;

    if (handled) {
      event.preventDefault();
      const snapshot = state.snapshot;
      if (!announced && snapshot?.message) announce(snapshot.message);
    }
  });
}

function setupControls() {
  const zone = els["drop-zone"];

  els["file-input"].addEventListener("change", () => {
    const files = Array.from(els["file-input"].files ?? []);
    if (files.length === 1) loadFile(files[0]);
    else if (files.length > 1) startBook(files);
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
    const files = Array.from(event.dataTransfer?.files ?? []);
    if (files.length === 1) loadFile(files[0]);
    else if (files.length > 1) startBook(files);
  });

  els["metadata-continue"].addEventListener("click", () => applyAction("confirm"));
  els["metadata-back"].addEventListener("click", bookBack);
  els["start-over"].addEventListener("click", startOver);
  els["triage-back"].addEventListener("click", bookBack);
  els["paste-use"].addEventListener("click", usePastedText);

  for (const button of els["triage-screen"].querySelectorAll("button[data-action]")) {
    button.addEventListener("click", () => {
      if (button.dataset.action === "auto_accept") {
        applyAutoAccept();
        return;
      }
      applyAction(button.dataset.action);
      const snapshot = state.snapshot;
      if (snapshot?.message) announce(snapshot.message);
    });
  }

  els["preview-back"].addEventListener("click", () => applyAction("back"));
  els["book-next"].addEventListener("click", bookForward);
  els["download-tex"].addEventListener("click", downloadTex);
  els["download-pdf"].addEventListener("click", downloadPdf);
  els["standalone-toggle"].addEventListener("change", () => announce(
    els["standalone-toggle"].checked
      ? "Standalone document enabled."
      : "Package song format selected."));
  els["transpose-down"].addEventListener("click", () => applyTranspose(-1));
  els["transpose-up"].addEventListener("click", () => applyTranspose(1));
  els["transpose-reset"].addEventListener("click", resetTranspose);

  els["book-form-continue"].addEventListener("click", startBookTriage);
  els["book-form-reset"].addEventListener("click", startOver);
  els["book-paste-add"].addEventListener("click", () => addPastedSong("form"));
  els["export-paste-add"].addEventListener("click", () => addPastedSong("export"));
  for (const id of ["book-title-input", "book-author-input"]) {
    const input = els[id];
    input.addEventListener("input", () => {
      const book = state.book;
      if (!book) return;
      // Raw strings: composeBook does the LaTeX escaping in core.
      book.title = els["book-title-input"].value;
      book.author = els["book-author-input"].value;
    });
    input.addEventListener("keydown", (event) => {
      if (event.key === "Enter") {
        event.preventDefault();
        els["book-form-continue"].focus();
      }
    });
  }
  els["chord-index-toggle"].addEventListener("change", () => {
    const book = state.book;
    if (!book) return;
    book.chordIndex = els["chord-index-toggle"].checked;
    bookStatus(book.chordIndex
      ? "Chord index included in the book."
      : "Chord index excluded from the book.");
  });
  els["book-export-tex"].addEventListener("click", exportBookTex);
  els["book-export-pdf"].addEventListener("click", downloadBookPdf);
  els["book-export-back"].addEventListener("click", bookExportBack);
  els["book-export-reset"].addEventListener("click", startOver);
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
