# Songbook system context

One page of shared context for architecture advice. Decisions live in
`docs/adr/`; the trade-offs behind them in `docs/principles.md`.

## What songbook is

A converter from chord-over-lyrics text files (`.txt`/`.crd`) to clean,
printable songs: LaTeX `songs`-package output, verified to PDF. Literal six-string
ASCII guitar tabs and associated diagram annotations are retained without musical
interpretation (ADR0021/0022). Non-technical collaborators triage messy input into
structured songs; the tool owns the typesetting.

## Context

```
 person + chord/lyric text files          printable songs
        │                                        ▲
        ▼                                        │
   ┌─────────────┐   .tex    ┌──────────────────┐ │
   │  songbook   │──────────▶│  TeX toolchain   │─┘
   │ (this repo) │           │ native Tectonic  │   (verification:
   └─────────────┘           │ or in-browser    │    ADR0007/0017)
        ▲                    │ Tectonic-wasm    │
        │                    └──────────────────┘
        └── song content never crosses the network (P3)
```

## Containers

| Container | Path | Role |
|---|---|---|
| `songbook-core` | `crates/songbook-core` | All semantics: `parser/` (chord, classify, song), `triage`, `latex/` (render), `preview`, `audit`, `book` (manifests + book rendering, ADR0018). Owns P1/P2. |
| CLI + TUI | `src/` (`main`, `batch`, `book`, `tui/`) | Interactive triage (ratatui), batch conversion, book compilation from plain-text manifests, non-interactive audit (ADR0013/0015/0018). Thin view over core. |
| wasm Session | `crates/songbook-wasm` | `Session` API over core: `snapshot`, `handle_action`/`handle_key`, `export_tex`. The browser's only doorway to semantics. |
| Web app | `web/` | Static, client-only vanilla JS (ADR0016): load file → metadata → triage → preview → `.tex` download; PDF via the lazy Tectonic-wasm engine (`web/pdf-engine.js` + worker + pinned assets, ADR0017). |
| PDF asset pipeline | `web/build-pdf-assets.sh`, `web/pdf/` | Hash-pinned, offline-replayable build of the engine + TeX pack (docstrip-generated `songs.sty`, BaKoMa CM). Determinism enforced by pins. |
| Devcontainer | `.devcontainer/` | The only environment with Rust + wasm-bindgen + Tectonic; builds wasm glue and runs native verification. |

## Data flow

```
.txt/.crd file
   → parser (chords, literal tabs/annotations, classification, source provenance)
   → triage (per-line decisions, tab annotation association; edits, split, undo)
   → metadata (title, author, key)
   → latex render (songs package; standalone or body)
   → .tex  ──▶ native Tectonic (dev/CI verification, byte-exact golden files)
           └─▶ browser Tectonic-wasm (user-facing PDF, same engine class)

A book is a plain-text manifest → `songbook-core::book` → one `.tex` that
composes each song's unchanged body, plus the `songs` package's title index
(ADR0018). In the browser, book PDFs run the same three-step pipeline through
the worker (compile → songidx port → compile), byte-exact against a native
reference (ADR0019).
```

## Literal tablature (ADR0021/0022)

Core owns literal tab recognition, annotation association, export validation,
shared warnings and split plans. `LineKind::Fretboard` string rows and
`LineKind::TabAnnotation` diagram comments have separate `TabRowRole` values;
annotations never count toward six strings, become lyrics, transpose or enter
chord indexes. Automatic association requires contiguous parenthetical
chord/fingering cues, optionally with one chord heading in either order. Explicit
**Tab annotation** association attaches adjacent comments nearest-first.

`TabProvenance` keeps song-local origin, block-instance, source-line and
role-specific row identities through review, edits, splitting and undo. Rebuilt
`Song::tab_groups` separates an annotation prefix from the retained string range;
source boundaries and distinct split instances prevent accidental group merging.
Original non-six runs cannot become supported through discards; partial original
six-string groups are retained with shared warnings, not filled with invented rows.

Core alone plans and atomically applies splits of complete supported blocks,
including their annotations, with stale-plan rejection and one-step undo. The
wasm Session keeps the executable plan private; snapshots expose only core bounds,
preview fragments, roles/provenance and warnings. TUI F2 and the web's contextual
split control present those decisions; JavaScript groups display context from
shared identities, never inferring source groups or slicing notation itself.

Export checks each literal annotation/string row for printable ASCII and at most
56 raw columns at 10pt cmtt. Fixed TeX structure, numeric glyph tokens and measured
space cells preserve alignment without executing source text as TeX. Tab-bearing
standalone songs and books use one song column. All PDF paths compile core's exact
artifact; there is no fret transposition, musical validation or new dependency.

## Fitness functions

- Golden-file tests over conversion output (ADR0008).
- The PDF smoke test: browser-engine output must match the native reference
  byte-for-byte in extracted text (ADR0017, `web/pdf/test/smoke-node.mjs`).
- Build pins: every upstream source and the built pack are sha256-pinned;
  mismatch fails the build (ADR0017).
- Golden book fixture: the committed `examples/demo-book.tex` is byte-exact,
  and the PDF smoke compiles a book fixture byte-exact against a native
  three-step reference (ADR0018, ADR0019).
- Synthetic tablature regressions cover annotation recognition/manual association,
  source and role identity, exact whitespace, export bounds/safe glyph rendering,
  unchanged transposition, split boundaries/rejection/staleness and atomic undo
  (ADR0021/0022).
- Tablature integration verification (2026-11-09): devcontainer workspace/wasm-target
  gates and browser build passed; seven browser-worker PDF cases matched native
  text/layout and all 18 tested native/browser raster pages were byte-identical,
  including annotated and split standalone/book output. Owner visual acceptance
  followed that day; automated interaction/accessibility checks do not claim a
  hands-on screen-reader test. Release remains pending.

## Invariants

- The same core semantics answer every face (P2).
- Song content never crosses the network; nothing persists by default (P3).
- The `.tex` is rendered once, by core, and compiled as-is everywhere (P1).
- `web/dist/` is a build artifact, never source; run workspace/wasm gates,
  browser builds and pinned PDF asset builds in the devcontainer.

## Where things live

- Architecture decisions: `docs/adr/` (ADR0000 explains the practice).
- Principles and sliders: `docs/principles.md`.
- Authoritative roadmap: native Git issues (`git issue ls --all`); see
  [README](../README.md#roadmap-native-git-issues). Historical roadmap context:
  `docs/roadmap.md`.
