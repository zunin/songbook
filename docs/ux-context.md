# UX context brief (for advisors)

Maintained by the architecture facilitator. Read this before any UX review of
songbook's interfaces. Last updated: 2026-11-03.

## What songbook is

A converter from chord-over-lyrics text files (`.txt`/`.crd`, typically
copied from tab websites) to clean, printable songs: LaTeX `songs`-package
output, compiled to PDF. Non-technical collaborators triage messy input into
structured songs; the tool owns the typesetting. Privacy by architecture:
everything runs client-side, nothing is uploaded or persisted.

## Who uses it

The project owner (technical, keyboard-first, uses the TUI and CLI) and
non-technical collaborators (browser only, mixed desktop environments, some
rely on screen readers). The web app is the public face (deployed at
zunin.github.io/songbook); the CLI/TUI is the power surface.

## Surfaces

1. **CLI** (`songbook file.txt`): interactive TUI triage, or `--yes` for
   non-interactive conversion with an audit; `--book` for multi-song books;
   `--transpose` for key shifts.
2. **TUI** (ratatui): metadata form → per-line triage (keep/discard/lyric/
   chord-only/header/noise/edit/undo/flags) → structure preview → export.
   Keyboard only; footer legends; status messages.
3. **Web app** (vanilla JS + wasm, no framework): load screen (file picker,
   drag-drop, paste textarea; single song or book with mixed file/pasted
   sources) → book details (optional) → metadata form → per-line triage
   (same verdicts as the TUI, plus visible choice labels like
   `lyric (was: ambiguous)`) → structure preview (transpose ±/reset,
   downloads) → book export. Screen-reader support: live regions,
   aria-keyshortcuts, visually-hidden text.

## Flows (web)

load → (book details) → metadata → triage (per song, sequential in book
mode) → preview → export/book export. "Start over" resets everything; no
persistence by design (ADR0016): closing the tab discards the session.

## Binding constraints for any UI change

- Vanilla JS/CSS, no dependencies, no build framework (ADR0016).
- Keyboard operable throughout; every control labelled; status through
  live regions; no color-only meaning.
- No storage APIs, no network beyond same-origin engine assets (privacy,
  ADR0016/0020).
- One vocabulary across web, TUI, core, and screen readers: `lyric`,
  `chord-only`, `section header`, `noise`, `fretboard`, `dropped`,
  `discarded`, `ambiguous`, `blank` (lowercase, from the core's
  `line_kind_label`). Buttons are Title-case actions (Keep, Discard, …) —
  actions ≠ state labels.
- Markers are a separate channel from labels: `· ? × ✎ ✓` say what still
  needs attention; labels say what a line is (pending = parser's guess,
  decided = the user's choice, with muted `(was: X)` provenance only on
  disagreement).

## Known limitations (accepted, don't re-flag unless worsened)

- No mobile/touch layout (desktop browsers only).
- No persistence/autosave; paste and triage decisions die with the tab.
- Orphan intro chords render as chord-only blocks (documented).
- No rich source editor in books (reorder/remove = start over).
- The TUI remains file-based (no paste).
- **TUI line editing is append-only** (closed decision, 2026-11-03): no cursor
  movement or mid-line insert/delete in the terminal editor; the edit legend
  says so, and mid-line edits belong to the web's inline editor.

## Interaction conventions (closed model, 2026-11-03)

- **Focus model (web triage)**: the line list keeps keyboard focus when it had
  it (the cursor-follows-verdict feedback); the actions group keeps focus when
  the user moved there (announcements carry the new line instead); entering the
  screen pulls focus to the current row once. No further focus redesign.
- **Markers vs labels**: markers say what still needs attention; labels say
  what a line is; provenance is muted and only on disagreement (see above).
- **Vocabulary**: one lowercase set across web, TUI, core, and screen readers.

## Where things live

- Parser/classifier: `crates/songbook-core/src/parser/classify.rs`
- Triage state machine: `crates/songbook-core/src/triage.rs`
- Web app: `web/app.js`, `web/index.html`, `web/styles.css`
- TUI: `src/tui/`
- wasm snapshot surface: `crates/songbook-wasm/src/{model,session}.rs`