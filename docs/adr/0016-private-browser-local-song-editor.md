# ADR0016 — Private browser-local song editor (static client-only app)

## Status

Adopted

## Decision

The songbook exposes a **browser-local MVP**: a static, client-only web app
(`web/`) that runs entirely in the browser. The product boundary approved by
the project owner:

- **No server processing.** There is no backend; the app is served as static
  files (HTML/CSS/JS + WASM). Conversion, triage, and LaTeX rendering all run
  client-side in WebAssembly, reusing the existing `songbook-wasm` session API
  over `songbook-core` — no parser, triage, or renderer logic is duplicated in
  JavaScript.
- **No upload of song text.** Song files are read with the browser File API
  and never sent over the network. No content analytics, no content
  telemetry, no logging.
- **No persistence by default.** Nothing is written to `localStorage`,
  `sessionStorage`, IndexedDB, or cookies. Closing the tab discards the
  session ("Start over" is explicit).
- **File-based input only for this MVP** (per ADR0005): choose/drop a
  `.txt`/`.crd` file. Paste input is deliberately excluded in this MVP and
  remains future work.
- **Output is a `.tex` download** produced from a `Blob` +
  `URL.createObjectURL`. PDF rendering is a later stretch goal; no PDF code is
  included.
- Target audience: non-technical collaborators on mixed desktop browsers, via
  a plain vanilla HTML/CSS/JS app with no frontend dependencies.

## Context

ADR0005 restricts songbook input to files (no paste), and ADR0014 requires
triage over every raw input line with the same semantics as the TUI. The
browser milestone (`songbook-wasm`) already exposes the core session API to
JavaScript; this ADR defines the product boundary under which the static web
app is built and distributed.

## Consequences

- The web app is a thin UI layer over the WASM session: it sends semantic
  actions/keys and renders returned snapshots. All parse/triage/rebuild/render
  semantics, including ADR0014's raw-line rebuild and ADR0012/0013 defaults,
  stay in core.
- The MVP flow mirrors the TUI: load file → metadata form → per-line triage
  (keep/discard/lyric/chord-only/header/noise, edit, undo, flag jumps,
  preview) → structure preview → `.tex` download.
- Generated WASM/JS glue (`web/dist/`) is a build artifact, not source; a
  repeatable local build script (`web/build.sh`) compiles
  `songbook-wasm` for `wasm32-unknown-unknown` and generates wasm-bindgen glue
  pinned to the crate's wasm-bindgen version.

## Limitations (accepted for this MVP)

- **No paste input** (ADR0005 boundary): files only, `.txt`/`.crd`.
- **No persistence/autosave**: closing the tab loses triage decisions.
- **No mobile/touch layout**: mixed desktop browsers only; no responsive
  touch-first design.
- **No PDF output**: `.tex` download only; PDF is a later stretch goal.
- **No batch mode, no multiple-file sessions** (one song per browser session).
- **No transposition, i18n, or accessibility polish beyond the baseline**
  (keyboard operation, live regions, color-independent status markers are in;
  full screen-reader flow testing is not).
- **Distribution needs a static host (or local server)**: browsers do not run
  ES modules/WASM from `file://` pages, so the bundle must be served over
  HTTP(S) or opened from a local static server. Embedding the WASM inline for
  single-file distribution is future work.
- **Undo granularity in metadata fields** follows the core session's
  character-level undo; whole-field undo is not exposed by the API yet.
