import init, { SongbookSession, composeBook } from "./songbook_wasm.js";

const ACCEPTED_EXTENSIONS = [".txt", ".crd"];
const UNCHANGED_TAB_WARNING =
  "Chord transposition is active; ASCII tabs are unchanged and may no longer match.";

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
  // What #triage-live last said for the current session (F11): the position,
  // the current line's status, and the pending count. The delta against it
  // decides the next announcement — unchanged info is never re-announced,
  // and a new song's first render starts fresh instead of comparing against
  // the previous song's counts.
  announced: null,
  // Warnings have their own polite channel; moving through unchanged rows
  // must not repeat them or lengthen the F11 verdict announcement.
  announcedWarnings: null,
  splitPreview: null,
  splitBounds: null,
};

const els = {};

function $(id) {
  return document.getElementById(id);
}

function initEls() {
  for (const id of [
    "load-screen", "metadata-screen", "triage-screen", "preview-screen",
    "drop-zone", "file-input", "file-error",
    "paste-input", "paste-use", "paste-book", "paste-error",
    "book-paste-input", "book-paste-add", "book-paste-error",
    "book-file-input", "book-song-list",
    "export-paste-input", "export-paste-add", "export-paste-error",
    "metadata-form", "metadata-continue", "metadata-error",
    "triage-status", "triage-live", "triage-list", "start-over",
    "triage-warnings", "triage-tab-scroll", "triage-tab-explanation",
    "split-tab-button", "split-tab-panel", "split-tab-column", "split-tab-error",
    "split-tab-range", "split-tab-scope", "split-tab-preview-trigger",
    "split-tab-preview", "split-tab-preview-live", "split-tab-left", "split-tab-right",
    "split-tab-left-heading", "split-tab-right-heading",
    "split-tab-left-empty", "split-tab-right-empty", "split-tab-row-details",
    "split-tab-warnings", "split-tab-apply", "split-tab-cancel",
    "preview-warnings", "preview-tab-help", "book-warnings", "warnings-live",
    "preview-content", "preview-back", "standalone-toggle", "download-tex",
    "download-pdf", "pdf-status", "convert-another",
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

// Unresolved lines: the same count the visual status bar reports. Pending
// flags are the flagged subset; "pending" here is every undecided line.
function countPending(snapshot) {
  return snapshot.lines.filter((line) => line.status === "pending").length;
}

function statusText(snapshot) {
  return `${snapshot.lineCount} lines · ${snapshot.flagCount} flagged · ` +
    `${countPending(snapshot)} pending · line ${snapshot.position + 1} current`;
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

// Roles/labels, not string/fret parsing: core alone decides which source rows
// are notation. Keep literal source legible even after Discard/reclassification.
function isTabReviewLine(line) {
  return line && (line.tabRole === "annotation" || line.tabRole === "string" ||
    line.classifier === "fretboard" || line.classifier === "tab annotation" ||
    line.effectiveLabel === "fretboard" || line.effectiveLabel === "tab annotation");
}

function tabReviewDetails(snapshot) {
  const groups = new Map();
  const details = new Map();
  for (const line of snapshot.lines) {
    if (line.tabRole !== "annotation" && line.tabRole !== "string") continue;
    details.set(line.index, { role: line.tabRole });
    if (line.tabOrigin == null || line.tabInstance == null) continue;
    const key = JSON.stringify([line.tabOrigin, line.tabInstance]);
    if (!groups.has(key)) groups.set(key, { annotation: [], string: [] });
    groups.get(key)[line.tabRole].push(line);
  }
  for (const group of groups.values()) {
    for (const role of ["annotation", "string"]) {
      const rows = group[role];
      // The full review snapshot includes discarded/reclassified rows. Count
      // by provenance, never by retained text or apparent string labels. Only
      // number strings when all six source identities are available; otherwise
      // the API would need original_row/original_rows to promise an ordinal.
      if (role === "string" && rows.length !== 6) continue;
      const sources = new Set(rows.map((line) => line.sourceLine));
      if (sources.size !== rows.length || rows.some((line) =>
        !Number.isInteger(line.sourceLine) || line.sourceLine < 1)) continue;
      rows.sort((a, b) => a.sourceLine - b.sourceLine);
      rows.forEach((line, index) => details.set(line.index, {
        role, ordinal: index + 1, count: role === "string" ? 6 : rows.length,
      }));
    }
  }
  return details;
}

function tabRoleText(detail) {
  if (!detail) return "";
  if (!detail.ordinal) return detail.role;
  return detail.role === "string"
    ? `string ${detail.ordinal}/${detail.count}` : `annotation ${detail.ordinal}`;
}

function reviewLabel(line, detail) {
  // "tab annotation" is core's effective kind. The view presents it alongside
  // fretboard strings, but explicit Lyric/Header/etc. overrides remain intact.
  return line.effectiveLabel === "tab annotation" && detail?.role === "annotation"
    ? "fretboard" : line.effectiveLabel;
}

function reviewLabelText(line, detail) {
  const role = tabRoleText(detail);
  return reviewLabel(line, detail) + (role ? ` · ${role}` : "");
}

function sourceIdentity(line) {
  if (!isTabReviewLine(line)) return "";
  const parts = [];
  if (line.sourceLine != null) parts.push(`source line ${line.sourceLine}`);
  if (line.tabInstance != null) parts.push(`tab group ${line.tabInstance}`);
  if (line.tabOrigin != null) parts.push(`origin ${line.tabOrigin}`);
  return parts.length ? `, ${parts.join(", ")}` : "";
}

function snapshotWarnings(snapshot) {
  return Array.isArray(snapshot.warnings) ? snapshot.warnings : [];
}

function renderWarningList(id, warnings, heading = "Song warnings") {
  const box = els[id];
  box.textContent = "";
  box.hidden = warnings.length === 0;
  if (box.hidden) return;
  const title = document.createElement("h3");
  title.textContent = heading;
  const list = document.createElement("ul");
  for (const warning of warnings) {
    const item = document.createElement("li");
    item.textContent = warning;
    list.append(item);
  }
  box.append(title, list);
}

function announceWarningChanges(warnings, owner, label = "Song") {
  const previous = state.announcedWarnings;
  const signature = JSON.stringify(warnings);
  if (previous?.owner === owner && previous.signature === signature) return;
  state.announcedWarnings = { owner, signature, count: warnings.length };
  els["warnings-live"].textContent = warnings.length > 0
    ? `${label} warnings: ${warnings.join(" ")}`
    : previous?.owner === owner && previous.count > 0
      ? `${label} warnings cleared.` : "";
}

function renderSongWarnings(snapshot, id) {
  const warnings = snapshotWarnings(snapshot);
  renderWarningList(id, warnings);
  announceWarningChanges(warnings, state.session);
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
    wrap.append(label);
    // The hint trails the field, reading "Title [input] (inferred)" — the
    // same {label} {value} (inferred) order the TUI metadata panel uses.
    if (field.inferred) {
      const hint = document.createElement("span");
      hint.className = "hint";
      hint.textContent = "(inferred)";
      wrap.append(hint);
    }
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
  const wasEditing = state.editing;
  state.editing = snapshot.editing;
  els["triage-back"].hidden = !state.book;
  if (state.book) {
    els["triage-back"].textContent = bookBackLabel();
  }
  els["triage-status"].textContent = statusText(snapshot);
  const currentIsTab = isTabReviewLine(snapshot.lines[snapshot.position]);
  els["triage-tab-explanation"].hidden = !currentIsTab;
  const editButton = els["triage-screen"].querySelector('button[data-action="edit"]');
  if (currentIsTab) editButton.setAttribute("aria-describedby", "triage-tab-edit");
  else editButton.removeAttribute("aria-describedby");
  renderSongWarnings(snapshot, "triage-warnings");
  renderTabSplitAction(snapshot);
  renderTriageList(snapshot, wasEditing);
}

function openTabSplit() {
  const bounds = state.snapshot?.tabSplitBounds;
  if (!bounds) return;
  state.splitBounds = bounds;
  state.splitPreview = null;
  els["split-tab-error"].textContent = "";
  els["split-tab-warnings"].textContent = "";
  els["split-tab-warnings"].hidden = true;
  els["split-tab-preview"].hidden = true;
  const input = els["split-tab-column"];
  input.min = String(bounds.minAfterColumn);
  input.max = String(bounds.maxAfterColumn);
  input.value = String(bounds.maxAfterColumn);
  els["split-tab-range"].textContent =
    `${bounds.minAfterColumn} through ${bounds.maxAfterColumn} inclusive`;
  const annotations = bounds.annotationCount ?? 0;
  els["split-tab-scope"].textContent =
    `${annotations} annotation ${annotations === 1 ? "row" : "rows"} above six string rows; ` +
    `review lines ${bounds.start + 1} through ${bounds.end}, original block source line ${bounds.sourceLine}.`;
  els["split-tab-preview-live"].textContent = "";
  els["split-tab-apply"].hidden = true;
  els["split-tab-preview-trigger"].hidden = false;
  els["split-tab-panel"].hidden = false;
  input.focus();
  announce(`Choose a raw column from ${bounds.minAfterColumn} through ${bounds.maxAfterColumn}.`);
}

function invalidateTabSplitPreview(announceChange = true) {
  if (!state.splitPreview) return;
  // Cancel only the private plan. Typing a new column never plans or applies
  // anything; even changing back to the old value requires explicit Preview.
  try {
    state.session.cancelTabSplit();
  } catch (error) {
    announce(`Could not clear split preview: ${error}`);
  }
  state.splitPreview = null;
  els["split-tab-preview"].hidden = true;
  els["split-tab-apply"].hidden = true;
  els["split-tab-preview-trigger"].hidden = false;
  els["split-tab-warnings"].textContent = "";
  if (announceChange) {
    els["split-tab-preview-live"].textContent =
      "Column changed. Preview split again before applying.";
  }
}

function previewTabSplit() {
  invalidateTabSplitPreview(false);
  const input = els["split-tab-column"];
  const value = input.valueAsNumber;
  const bounds = state.splitBounds;
  if (!bounds || !Number.isInteger(value) || value < bounds.minAfterColumn ||
      value > bounds.maxAfterColumn) {
    els["split-tab-error"].textContent = bounds
      ? `Enter a whole raw column from ${bounds.minAfterColumn} through ${bounds.maxAfterColumn}.`
      : "The selected tab block is no longer eligible. Cancel and select a complete original six-string block.";
    input.focus();
    return;
  }
  try {
    state.splitPreview = state.session.planTabSplit(value);
    els["split-tab-error"].textContent = "";
    renderTabSplitPreview(state.splitPreview);
  } catch (error) {
    state.splitPreview = null;
    els["split-tab-preview"].hidden = true;
    els["split-tab-apply"].hidden = true;
    els["split-tab-error"].textContent =
      `Could not create split preview: ${error}. Review the selected block and try again, or cancel.`;
    announce(els["split-tab-error"].textContent);
  }
}

function cancelTabSplit() {
  try {
    state.snapshot = state.session.cancelTabSplit();
  } catch (error) {
    announce(`Could not cancel split preview: ${error}`);
  }
  state.splitPreview = null;
  state.splitBounds = null;
  els["split-tab-panel"].hidden = true;
  render();
  els["split-tab-button"].focus({ preventScroll: true });
  announce("Split preview cancelled; the song is unchanged.");
}

function applyTabSplit() {
  if (!state.splitPreview) return;
  if (els["split-tab-column"].valueAsNumber !== state.splitPreview.afterColumn) {
    invalidateTabSplitPreview();
    els["split-tab-error"].textContent =
      "The column differs from the preview. Preview split again before applying.";
    return;
  }
  try {
    state.snapshot = state.session.applyTabSplit();
    state.splitPreview = null;
    state.splitBounds = null;
    els["split-tab-panel"].hidden = true;
    render();
    announce("Tab block split applied as one undoable change.");
  } catch (error) {
    state.splitPreview = null;
    els["split-tab-preview"].hidden = true;
    els["split-tab-error"].textContent =
      `The split plan is stale or could not be applied: ${error}. No split was applied. Cancel and select the block again to make a fresh preview.`;
    els["split-tab-apply"].hidden = true;
    els["split-tab-preview-trigger"].hidden = false;
    announce(els["split-tab-error"].textContent);
  }
}

function renderTabSplitAction(snapshot) {
  const bounds = snapshot.tabSplitBounds ?? null;
  const currentIndex = snapshot.position;
  const eligible = !snapshot.editing && bounds &&
    currentIndex >= bounds.start && currentIndex < bounds.end;
  els["split-tab-button"].hidden = !eligible;
  if (!eligible && !state.splitPreview) {
    els["split-tab-panel"].hidden = true;
    state.splitBounds = null;
    return;
  }
  if (eligible) state.splitBounds = bounds;
  if (state.splitPreview) renderTabSplitPreview(state.splitPreview);
}

function renderTabSplitSide(preview, side) {
  const annotations = preview.annotations ?? [];
  const box = els[`split-tab-${side}`];
  const name = side === "left" ? "Left" : "Right";
  const count = `${annotations.length} annotation ${annotations.length === 1 ? "row" : "rows"} above ${preview.rows.length} string rows`;
  els[`split-tab-${side}-heading`].textContent = `${name} block — ${count}`;
  box.setAttribute("aria-label", `${name} split preview, ${count}, scrollable`);
  box.textContent = "";
  const empty = [];
  for (const [role, rows] of [["annotation", annotations], ["string", preview.rows]]) {
    for (const row of rows) {
      const literal = document.createElement("div");
      literal.className = `split-literal-fragment split-${role}`;
      literal.textContent = row[side];
      box.append(literal);
      if (role === "annotation" && (side === "right"
        ? row.rightLabelOnly : row.leftColumns === 0)) {
        empty.push(`Annotation ${row.originalRow}, source line ${row.sourceLine}: ` +
          (side === "right"
            ? `no text after the cut; the literal row contains only ${row.paddingColumns} alignment spaces.`
            : "empty literal fragment."));
      }
    }
  }
  // Describe an empty suffix outside the literal rows. Never insert placeholder
  // characters into core's text or between annotations and their six strings.
  const placeholder = els[`split-tab-${side}-empty`];
  placeholder.hidden = empty.length === 0;
  placeholder.textContent = empty.join(" ");
}

function renderTabSplitPreview(preview) {
  renderTabSplitSide(preview, "left");
  renderTabSplitSide(preview, "right");
  const details = els["split-tab-row-details"];
  details.textContent = "";
  for (const [role, rows] of [["annotation", preview.annotations ?? []], ["string", preview.rows]]) {
    for (const row of rows) {
      const item = document.createElement("li");
      const label = role === "annotation"
        ? `Annotation ${row.originalRow}` : `String ${row.originalRow}/6`;
      item.textContent = `${label}, source line ${row.sourceLine}: ` +
        `original ${row.originalColumns} columns; left ${row.leftColumns}; right ${row.rightColumns}. ` +
        `${row.paddingColumns} added right alignment spaces; ` +
        (role === "annotation" ? "no annotation text repeated." :
          `${row.prefix.length} exact label/first-pipe prefix columns repeated.`);
      details.append(item);
    }
  }
  els["split-tab-warnings"].textContent = preview.warnings.join(" ");
  els["split-tab-warnings"].hidden = preview.warnings.length === 0;
  els["split-tab-preview"].hidden = false;
  els["split-tab-apply"].hidden = false;
  els["split-tab-preview-trigger"].hidden = false;
  els["split-tab-preview-live"].textContent =
    `Split preview after raw column ${preview.afterColumn}. Each child has ` +
    `${(preview.annotations ?? []).length} annotation rows above ${preview.rows.length} string rows. ` +
    `Left block, then right block. Row lengths and added alignment spaces are disclosed below. ${preview.warnings.join(" ")}`;
}

function renderTriageList(snapshot, wasEditing) {
  const list = els["triage-list"];
  // Focus rule (UX review F2): the pull exists so a screen reader announces
  // the row a verdict moved the cursor to — it must not yank focus out of
  // the action buttons after every render. Pull only when focus already
  // lived in the list (the rebuild destroyed it), when this screen just took
  // over (the previous screen's focus died with it), or when edit mode just
  // opened (the input must own focus to be typed into). Otherwise keep
  // focus where it is and let announceTriageChange carry the change.
  const focusOnList = document.activeElement === list;
  const focusInList = list.contains(document.activeElement);
  const focusOnScreen = els["triage-screen"].contains(document.activeElement);
  const enteringEdit = snapshot.editing && !wasEditing;
  const horizontal = list.scrollLeft;
  const hasTabs = snapshot.lines.some(isTabReviewLine);
  els["triage-tab-scroll"].hidden = !hasTabs;
  if (hasTabs) {
    list.tabIndex = 0;
    list.setAttribute("aria-label", "Song lines, scrollable");
    list.setAttribute("aria-describedby", "triage-tab-scroll");
  } else {
    list.removeAttribute("tabindex");
    list.removeAttribute("aria-label");
    list.removeAttribute("aria-describedby");
  }

  const tabDetails = tabReviewDetails(snapshot);
  list.textContent = "";
  for (const line of snapshot.lines) {
    const detail = tabDetails.get(line.index);
    const item = document.createElement("div");
    item.setAttribute("role", "listitem");
    item.dataset.index = String(line.index);
    item.className = "triage-line";
    if (isTabReviewLine(line)) item.classList.add("tab-line");
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
    kind.textContent = reviewLabel(line, detail);
    if (detail) {
      const role = document.createElement("span");
      role.className = "line-tab-role";
      role.textContent = ` · ${tabRoleText(detail)}`;
      kind.append(role);
    }
    if (line.effectiveLabel !== line.classifier) {
      const was = document.createElement("span");
      was.className = "was";
      was.textContent = wasSuffix(line);
      kind.append(was);
    }

    const described = `status ${line.status}` +
      `${line.flag ? ", flagged" : ""}${line.index === snapshot.position ? ", current" : ""}`;

    if (state.editing && line.index === snapshot.position) {
      item.append(marker, number, kind, buildEditInput(line, detail));
    } else {
      const select = document.createElement("button");
      select.type = "button";
      select.className = "line-select";
      if (isTabReviewLine(line)) {
        select.setAttribute("aria-describedby", "triage-tab-scroll triage-tab-keep");
      }
      select.setAttribute("aria-label",
        `Line ${line.index + 1}, ${reviewLabelText(line, detail)}${wasSuffix(line)}${sourceIdentity(line)}, ${described}: ` +
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
  // A deliberately focused scroll container stays focused for native
  // Left/Right scrolling; row and action-button focus still follow F2.
  const keepListFocus = focusOnList && hasTabs && !enteringEdit;
  const target = keepListFocus ? list : current &&
    (current.querySelector(".line-edit-input") ??
      current.querySelector(".line-select"));
  const pullFocus = Boolean(target) &&
    (focusInList || !focusOnScreen || enteringEdit);
  if (pullFocus && document.activeElement !== target) {
    target.focus({ preventScroll: true });
  }
  // Follow the current row even when focus stays on the scroller or actions.
  // Scroll only the list vertically, never the page or a long tab row sideways.
  if (current) {
    const rect = current.getBoundingClientRect();
    const top = list.getBoundingClientRect().top + list.clientTop;
    const bottom = top + list.clientHeight;
    if (rect.top < top) list.scrollTop += rect.top - top;
    else if (rect.bottom > bottom) list.scrollTop += rect.bottom - bottom;
  }
  // Rebuilding the list can reset horizontal scroll; keep the user's position.
  list.scrollLeft = horizontal;
  announceTriageChange(snapshot, pullFocus && !keepListFocus, tabDetails);
}

// F11: one SHORT, change-focused message per action in #triage-live, built
// from what actually changed since this session's last triage render — the
// visual status bar (#triage-status) is plain text now and is never
// announced. The parts, in speech order:
// - the session's own message ("Undone.", "No more flagged lines."): the
//   feedback for actions that changed nothing else,
// - the line the cursor is on now ("Line 12, lyric, kept") — skipped when
//   the focus pull (F2) already made the screen reader read that row, and
//   skipped when neither the position nor the current line's status changed,
//   so a no-op never re-announces,
// - "N pending left" when a verdict (or undo) changed the pending count.
function announceTriageChange(snapshot, pullFocus, tabDetails) {
  const line = snapshot.lines[snapshot.position];
  const label = line
    ? reviewLabelText(line, tabDetails.get(line.index)) + sourceIdentity(line) : "";
  const pending = countPending(snapshot);
  const fresh = state.announced !== null &&
    state.announced.session === state.session;
  const lineChanged = !fresh ||
    state.announced.position !== snapshot.position ||
    state.announced.status !== (line ? line.status : null) ||
    state.announced.label !== label;
  const countChanged = fresh && state.announced.pending !== pending;
  state.announced = {
    session: state.session,
    position: snapshot.position,
    status: line ? line.status : null,
    label,
    pending,
  };
  const parts = [];
  if (snapshot.message) parts.push(snapshot.message);
  if (!pullFocus && lineChanged && line) {
    let lineText = `Line ${snapshot.position + 1}, ${label}, ` +
      `${line.status}`;
    if (countChanged) lineText += `, ${pending} pending left`;
    parts.push(lineText);
  } else if (countChanged) {
    parts.push(`${pending} pending left`);
  }
  if (parts.length > 0) announce(parts.join(" "));
}

function buildEditInput(line, detail) {
  const input = document.createElement("input");
  input.type = "text";
  input.className = "line-edit-input";
  input.value = state.snapshot.editBuffer ?? state.snapshot.lines[line.index].text;
  input.setAttribute("aria-label",
    `Editing line ${line.index + 1}, ${reviewLabelText(line, detail)}${sourceIdentity(line)}`);
  if (isTabReviewLine(line)) {
    input.setAttribute("aria-describedby", "triage-tab-scroll triage-tab-edit");
    input.spellcheck = false;
  }
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
    // Enter saves, Escape cancels; Tab behaves natively (UX review F6) —
    // tabbing away must not silently commit a half-typed edit.
    if (event.key === "Enter") {
      event.preventDefault();
      applyAction("confirm");
    } else if (event.key === "Escape") {
      event.preventDefault();
      applyAction("escape");
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
  if (row) row.focus({ preventScroll: true });
}

function renderPreviewPhase(snapshot) {
  const container = els["preview-content"];
  // Clicking a transpose control re-renders the preview; keep focus on the
  // button instead of yanking it into the scroll container.
  const previewHadFocus = container.contains(document.activeElement) ||
    TRANSPOSE_IDS.some((id) => els[id] === document.activeElement);
  const oldBlocks = Array.from(container.querySelectorAll(".tab-block"));
  const focusedBlock = oldBlocks.find((block) => block === document.activeElement);
  const horizontal = new Map(oldBlocks.map((block) =>
    [block.dataset.previewStart, block.scrollLeft]));
  showScreen("preview-screen");
  const book = Boolean(state.book);
  els["standalone-toggle"].closest("label").hidden = book;
  els["download-tex"].hidden = book;
  els["download-pdf"].hidden = book;
  els["pdf-status"].hidden = book;
  // The single-song flow's forward exit (UX review F14): after a download
  // the next sentence in the user's head is "convert another song", not
  // "start over". Book mode has its own forward exit (Next song / Continue
  // to book export), so the button hides there.
  els["convert-another"].hidden = book;
  els["book-next"].hidden = !book;
  if (state.book) {
    els["book-next"].textContent = bookForwardLabel();
  }
  renderTransposeLabel(snapshot);
  renderSongWarnings(snapshot, "preview-warnings");
  const rows = sessionPreviewRows();
  const warningNotes = new Set(snapshotWarnings(snapshot));
  warningNotes.add(UNCHANGED_TAB_WARNING);
  const logicalRows = [];
  const blocks = [];
  let tabBlock = null;
  container.textContent = "";
  rows.forEach((row, index) => {
    // Core's blank/group-note/style rows delimit runs. Annotations share the
    // following literal strings' scroll region; a later note stays outside it.
    const literalTab = row.style === "tablature" || row.style === "tab-annotation";
    if (!literalTab) tabBlock = null;
    // Warnings have one visible list and one live channel, not duplicate notes
    // inside the preview. Keep the original core row indices for scrolling.
    if (row.style === "note" && warningNotes.has(row.text)) {
      logicalRows.push(null);
      return;
    }
    const div = document.createElement("div");
    div.className = `row-${row.style}`;
    div.dataset.previewIndex = String(index);
    div.textContent = row.text;
    logicalRows.push(div);
    // F12: preserve the preview's heading outline.
    if (row.style === "header") {
      div.setAttribute("role", "heading");
      div.setAttribute("aria-level", "2");
    } else if (row.style === "section") {
      div.setAttribute("role", "heading");
      div.setAttribute("aria-level", "3");
    }
    if (literalTab) {
      if (!tabBlock) {
        tabBlock = document.createElement("div");
        tabBlock.className = "tab-block";
        tabBlock.dataset.previewStart = String(index);
        tabBlock.tabIndex = 0;
        tabBlock.setAttribute("role", "region");
        tabBlock.setAttribute("aria-label", `ASCII tab block ${blocks.length + 1}, scrollable`);
        tabBlock.setAttribute("aria-describedby", "preview-tab-help");
        blocks.push(tabBlock);
        container.append(tabBlock);
      }
      tabBlock.append(div);
    } else {
      container.append(div);
    }
  });
  els["preview-tab-help"].hidden = blocks.length === 0;
  for (const [index, block] of blocks.entries()) {
    const annotations = block.querySelectorAll(".row-tab-annotation").length;
    const strings = block.querySelectorAll(".row-tablature").length;
    block.setAttribute("aria-label",
      `ASCII tab block ${index + 1}, ${annotations} annotation rows and ${strings} string rows, scrollable`);
    block.scrollLeft = horizontal.get(block.dataset.previewStart) ?? 0;
  }
  // Nested tab rows and omitted warning notes still use core's logical row
  // index, not the number of top-level DOM children. Row geometry includes
  // block borders/padding, and skips an omitted note to the next visible row.
  const first = logicalRows.find(Boolean);
  const target = logicalRows.slice(snapshot.previewScroll).find(Boolean);
  container.scrollTop = first && target
    ? target.getBoundingClientRect().top - first.getBoundingClientRect().top
    : container.scrollHeight;
  const restoredBlock = focusedBlock && blocks.find((block) =>
    block.dataset.previewStart === focusedBlock.dataset.previewStart);
  if (restoredBlock) restoredBlock.focus({ preventScroll: true });
  else if (!previewHadFocus || focusedBlock) container.focus({ preventScroll: true });
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
// sources in the order they were added, files in selection order — and that
// one array order is the triage order and the book part order alike. Failed
// sources are marked, reported, and excluded — never dropped silently.

// The book state shape: sources in add order (triage order and book part
// order alike), plus the title/author fields and the navigation epoch.
function newBook(results) {
  return {
    results,
    current: -1,
    title: "",
    author: "",
    chordIndex: false,
    // Navigation epoch: in-flight file loads must not yank the user after
    // they have moved to another book screen.
    nav: 0,
  };
}

// A book source (ADR0020) is either a chosen file or pasted tab text held in
// `text`. Only the raw source is stored; a session is created when triage
// reaches the entry, so a source the engine cannot use fails there, with the
// same per-entry report either way.
function fileEntry(file) {
  return { kind: "file", file, text: null, name: file.name,
    session: null, error: null, loading: false };
}

function pasteEntry(text, name) {
  return { kind: "paste", file: null, text, name,
    session: null, error: null, loading: false };
}

function startBook(files) {
  state.book = newBook(files.map(fileEntry));
  showBookForm();
}

// F7: a book can start from pasted text alone. The paste becomes the book's
// first pasted source and the flow continues on the book-details screen,
// where more pasted songs (and files) can be added before Start triage.
// Only the empty-text check happens here, as on the single-song path; text
// the engine cannot use fails per entry when triage reaches it, like any
// other book source.
function startBookFromPaste() {
  const raw = els["paste-input"].value;
  if (raw.trim().length === 0) {
    els["paste-error"].textContent =
      "Paste some tab text first, or choose a file above.";
    return;
  }
  state.book = newBook([]);
  state.book.results.push(pasteEntry(raw, nextPasteName(state.book)));
  els["paste-input"].value = "";
  els["paste-error"].textContent = "";
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
  book.results.push(pasteEntry(raw, nextPasteName(book)));
  input.value = "";
  error.textContent = "";
  if (screen === "form") {
    els["book-form-status"].textContent = bookFormStatusText(book);
    renderBookSongList(book);
    input.focus();
  } else {
    bookForward(); // triage the appended song right away
  }
}

// F7: files can join a book after a paste-only start, so books can be mixed
// in either direction. These are the same file sources the load screen
// creates; unsupported or unreadable files fail per entry when triage
// reaches them, exactly as for files chosen there.
function addBookFiles() {
  const book = state.book;
  const files = Array.from(els["book-file-input"].files ?? []);
  if (!book || files.length === 0) return;
  for (const file of files) book.results.push(fileEntry(file));
  els["book-file-input"].value = "";
  els["book-form-status"].textContent = bookFormStatusText(book);
  renderBookSongList(book);
}

function showBookForm() {
  const book = state.book;
  if (!book) return;
  book.nav += 1;
  // The form sits "before" every song. Start triage recomputes where to
  // resume — the first unfinished song (F8) — so the cursor just parks at
  // -1 here.
  book.current = -1;
  els["book-title-input"].value = book.title;
  els["book-author-input"].value = book.author;
  showScreen("book-form-screen");
  els["book-form-status"].textContent = bookFormStatusText(book);
  renderBookSongList(book);
  els["book-title-input"].focus();
}

// The first source that still needs triage (F8): failed sources are skipped
// exactly like bookForward skips them, and a source counts as completed only
// when its session has nothing pending — a session exists for every song
// triage has reached, so "has a session" alone cannot tell done from
// in-progress, and neither can the phase (accept-rest and preview both leave
// pending lines behind). -1 when every source is done or failed.
function bookResumeIndex(book) {
  for (let i = 0; i < book.results.length; i += 1) {
    const entry = book.results[i];
    if (entry.error) continue;
    if (entryUnfinished(entry)) return i;
  }
  return -1;
}

// A source is unfinished when triage never reached it (no session) or its
// session still has pending lines.
function entryUnfinished(entry) {
  return !entry.session ||
    entry.session.snapshot().lines.some((line) => line.status === "pending");
}

// Per-song status on the book-details screen (F8): ✓ done, ▸ current (where
// Start triage resumes), · pending, × skipped. Read-only — navigation stays
// in the triage screens' Back/Next flow.
function renderBookSongList(book) {
  const list = els["book-song-list"];
  const resume = bookResumeIndex(book);
  list.textContent = "";
  book.results.forEach((entry, index) => {
    const item = document.createElement("li");
    let mark;
    if (entry.error) mark = "× skipped";
    else if (entryUnfinished(entry)) {
      mark = index === resume ? "▸ current" : "· pending";
    } else mark = "✓ done";
    item.textContent = `${mark} — ${entry.name}`;
    list.append(item);
  });
}

function startBookTriage() {
  const book = state.book;
  if (!book) return;
  // Raw strings: composeBook does the LaTeX escaping in core.
  book.title = els["book-title-input"].value;
  book.author = els["book-author-input"].value;
  // Resume where the user left off (F8): the first source that is neither
  // completed (session with nothing pending) nor failed — not always song 1.
  // bookForward walks from current + 1, so park just before it; with every
  // source done or failed (-1) it goes straight to book export.
  const resume = bookResumeIndex(book);
  book.current = resume === -1 ? book.results.length - 1 : resume - 1;
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

function renderBookWarnings(book) {
  // Always ask each session for its current decisions/transposition, including
  // revisited songs. Only announcement signatures are cached, not snapshots.
  const warnings = book.results.filter((entry) => entry.session && !entry.error)
    .flatMap((entry) => snapshotWarnings(entry.session.snapshot())
      .map((warning) => `${entry.name}: ${warning}`));
  renderWarningList("book-warnings", warnings, "Book warnings");
  announceWarningChanges(warnings, book, "Book");
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
  renderBookWarnings(book);
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
  renderBookWarnings(book);
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
  state.announced = null;
  state.announcedWarnings = null;
  els["warnings-live"].textContent = "";
  for (const id of ["triage-warnings", "preview-warnings", "book-warnings"]) {
    renderWarningList(id, []);
  }
  els["metadata-form"].textContent = "";
  els["file-input"].value = "";
  els["file-error"].textContent = "";
  els["paste-input"].value = "";
  els["paste-error"].textContent = "";
  els["book-paste-input"].value = "";
  els["book-paste-error"].textContent = "";
  els["book-file-input"].value = "";
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
    // Escape cancels a staged split even while the numeric input owns focus;
    // the generic input guard below intentionally ignores ordinary shortcuts.
    if (!els["split-tab-panel"].hidden && event.key === "Escape") {
      event.preventDefault();
      cancelTabSplit();
      return;
    }
    const target = event.target;
    if (target instanceof HTMLInputElement ||
        target instanceof HTMLTextAreaElement ||
        target instanceof HTMLSelectElement) {
      return;
    }
    if (state.splitPreview || !els["split-tab-panel"].hidden) {
      if (event.key === "Escape") {
        event.preventDefault();
        cancelTabSplit();
      }
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
    // Every handled action's spoken feedback comes from the render
    // (announceTriageChange, F11) or from its own announcement
    // (applyTranspose, applyAutoAccept) — never from a second call here.
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
    else if (key === "t") {
      if (state.snapshot?.phase === "triage") applyAction("tab_annotation");
      else handled = false;
    }
    else if (key === "e") applyAction("edit");
    else if (key === "u") applyAction("undo");
    else if (key === "a") applyAutoAccept();
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
    else {
      // Folders (and any other drop the browser reports as zero files) must
      // not be a silent no-op (UX review F13).
      els["file-error"].textContent =
        "That doesn't look like a .txt/.crd file — choose files, not folders.";
    }
  });

  els["metadata-continue"].addEventListener("click", () => applyAction("confirm"));
  els["metadata-back"].addEventListener("click", bookBack);
  els["start-over"].addEventListener("click", startOver);
  els["triage-back"].addEventListener("click", bookBack);
  els["paste-use"].addEventListener("click", usePastedText);
  els["paste-book"].addEventListener("click", startBookFromPaste);

  for (const button of els["triage-screen"].querySelectorAll("button[data-action]")) {
    button.addEventListener("click", () => {
      if (button.dataset.action === "auto_accept") {
        applyAutoAccept();
        return;
      }
      // Feedback for every handled action is announceTriageChange's job
      // (F11): one composed message per action, no second call here.
      applyAction(button.dataset.action);
    });
  }

  els["split-tab-button"].addEventListener("click", openTabSplit);
  els["split-tab-preview-live"].addEventListener("keydown", (event) => {
    if (event.key === "Escape") {
      event.preventDefault();
      cancelTabSplit();
    }
  });
  els["split-tab-preview-trigger"].addEventListener("click", previewTabSplit);
  els["split-tab-column"].addEventListener("input", () => invalidateTabSplitPreview());
  els["split-tab-column"].addEventListener("keydown", (event) => {
    if (event.key === "Enter") {
      event.preventDefault();
      previewTabSplit();
    }
  });
  els["split-tab-apply"].addEventListener("click", applyTabSplit);
  els["split-tab-cancel"].addEventListener("click", cancelTabSplit);
  els["preview-back"].addEventListener("click", () => applyAction("back"));
  els["book-next"].addEventListener("click", bookForward);
  els["download-tex"].addEventListener("click", downloadTex);
  els["download-pdf"].addEventListener("click", downloadPdf);
  els["convert-another"].addEventListener("click", startOver);
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
  els["book-file-input"].addEventListener("change", addBookFiles);
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
