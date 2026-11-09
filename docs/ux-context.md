# UX context brief (for advisors)

Maintained by the architecture facilitator. Read this before any UX review of
songbook's interfaces. Last updated: 2026-11-09.

## What songbook is

A converter from chord-over-lyrics text files (`.txt`/`.crd`, typically
copied from tab websites) to clean, printable songs: LaTeX `songs`-package
output, compiled to PDF. Literal labelled six-string ASCII guitar tabs and
associated diagram annotations are retained without musical interpretation
(ADR0021/0022). Non-technical collaborators triage messy input into structured
songs; the tool owns the typesetting. Privacy by architecture: everything runs
client-side, nothing is uploaded or persisted.

## Who uses it

The project owner (technical, keyboard-first, uses the TUI and CLI) and
non-technical collaborators (browser only, mixed desktop environments, some
rely on screen readers). The web app is the public face (deployed at
zunin.github.io/songbook); the CLI/TUI is the power surface. The live website
is currently v0.2.2: annotated tabs and splitting are accepted in source, with
release and public deployment still pending.

## Surfaces

1. **CLI** (`songbook file.txt`): interactive TUI triage, or `--yes` for
   non-interactive conversion with an audit; `--book` for multi-song books;
   `--transpose` for key shifts.
2. **TUI** (ratatui): metadata form → per-line triage (keep/discard/lyric/
   chord-only/header/noise/tab annotation/edit/undo/flags) → structure preview
   → export. F2 opens the separate tab-block split flow. Keyboard only; footer
   legends; status messages.
3. **Web app** (vanilla JS + wasm, no framework): load screen (file picker,
   drag-drop, paste textarea; single song or book with mixed file/pasted
   sources) → book details (optional) → metadata form → per-line triage
   (same verdicts as the TUI, plus visible choice labels like
   `lyric (was: ambiguous)`) → structure preview (transpose ±/reset,
   downloads) → book export. Eligible triage rows expose **Split tab block**.
   Screen-reader support: live regions, aria-keyshortcuts, visually-hidden text.

## Flows (web)

load → (book details) → metadata → triage (per song, sequential in book
mode) → preview → export/book export. "Start over" resets everything; no
persistence by design (ADR0016): closing the tab discards the session.

## Literal tab review and splitting (ADR0021/0022)

- Keep / Discard / Edit / Undo remain **per row**. String rows retain the
  `fretboard` label; literal comments use `tab annotation`. Shared role and
  provenance data distinguish annotation rows above the six separate strings;
  annotations are not lyrics, chord-only content or section headers. Removing
  an annotation does not mean a missing string; orphan annotations have a shared
  warning.
- Automatic annotation recognition needs contiguous parenthetical chord/fingering
  cues, with an optional chord heading in either order. Ordinary parenthetical
  lyrics and lone headings are insufficient. **Tab annotation** (`t`) explicitly
  attaches a selected comment immediately above a complete original six-string
  block; attach several nearest-first, then work upward. Only already associated
  annotations may intervene. **Header** remains the section-header action.
- Literal annotation/string rows do not wrap: horizontal pan/scroll preserves
  spaces and alignment. Web review and preview provide focusable scroll regions,
  Left/Right navigation guidance and separate role descriptions. Keep/Discard
  explanations state that only the selected row changes; Edit warns about shared
  alignment. ASCII notation is not an accessible musical interpretation.
- **F2** in TUI triage and contextual **Split tab block** in web triage open a
  labelled numeric-entry → exact left/right preview → Apply / Cancel flow on
  eligible annotation or string rows. Raw columns include indentation, labels
  and the first pipe, not musical positions. The TUI numeric prompt uses Enter
  to preview, then Enter to Apply; web provides separate Preview split and Apply
  split buttons. Ordinary line editing still uses **Enter = Save**.
- Both previews place annotations above their six strings. Disclose repeated
  string prefixes, added alignment spaces, label-only right rows, empty annotation
  fragments and any remaining overflow; do not duplicate whole comments or insert
  placeholders into literal content. A cut may divide annotation text or notation:
  **no musically safe cut is guaranteed**. Changing the column requires a fresh
  preview; core rejects stale plans without mutation. Escape/Cancel changes
  nothing; Apply replaces the block atomically and Undo restores it in one step.
- Shared warnings cover partial six-string groups, ASCII/56-column export limits,
  unsupported original non-six runs and unchanged tabs under chord transposition.
  Show warnings visibly and announce status without color-only meaning. There is
  no fret transposition, tuning validation or musical validation.

The owner visually accepted annotated-tab inclusion and splitting on 2026-11-09:
“It looks great! No notes”. Automated interaction/accessibility checks and owner
review are the recorded evidence; no hands-on screen-reader test is claimed.

## Binding constraints for any UI change

- Vanilla JS/CSS, no dependencies, no build framework (ADR0016).
- Keyboard operable throughout; every control labelled; status through
  live regions; no color-only meaning.
- No storage APIs, no network beyond same-origin engine assets (privacy,
  ADR0016/0020).
- One vocabulary across web, TUI, core, and screen readers: `lyric`,
  `chord-only`, `section header`, `noise`, `fretboard`, `tab annotation`, `dropped`,
  `discarded`, `ambiguous`, `blank` (lowercase, from the core's
  `line_kind_label`). Buttons are Title-case actions (Keep, Discard, …) —
  actions ≠ state labels.
- Markers are a separate channel from labels: `· ? × ✎ ✓` say what still
  needs attention; labels say what a line is (pending = parser's guess,
  decided = the user's choice, with muted `(was: X)` provenance only on
  disagreement).

## Known limitations (accepted, don't re-flag unless worsened)

- Literal tabs are six-string guitar only, printable ASCII and at most 56 raw
  columns per annotation/string row at readable 10pt cmtt; no automatic wrapping,
  shrinking, bass support or musical interpretation. Partial retained groups are
  warned about, not completed; a split may still leave export-blocking overflow.
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