# Songbook system context

One page of shared context for architecture advice. Decisions live in
`docs/adr/`; the trade-offs behind them in `docs/principles.md`.

## What songbook is

A converter from chord-over-lyrics text files (`.txt`/`.crd`) to clean,
printable songs: LaTeX `songs`-package output, verified to PDF. Non-technical
collaborators triage messy input into structured songs; the tool owns the
typesetting.

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
   → parser (chords, lines, classification)
   → triage (keep/discard/lyric/chord-only/header/noise; edits, undo)
   → metadata (title, author, key)
   → latex render (songs package; standalone or body)
   → .tex  ──▶ native Tectonic (dev/CI verification, byte-exact golden files)
           └─▶ browser Tectonic-wasm (user-facing PDF, same engine class)

A book is a plain-text manifest → `songbook-core::book` → one `.tex` that
composes each song's unchanged body, plus the `songs` package's title index
(ADR0018). In the browser, book PDFs will need the package's three-step index
pipeline (`songidx.lua`) or a pre-seeded `.sbx` — a Phase 2 engine work item.
```

## Fitness functions

- Golden-file tests over conversion output (ADR0008).
- The PDF smoke test: browser-engine output must match the native reference
  byte-for-byte in extracted text (ADR0017, `web/pdf/test/smoke-node.mjs`).
- Build pins: every upstream source and the built pack are sha256-pinned;
  mismatch fails the build (ADR0017).
- Golden book fixture: the committed `examples/demo-book.tex` is byte-exact,
  and the PDF smoke also compiles a book fixture (ADR0018).

## Invariants

- The same core semantics answer every face (P2).
- Song content never crosses the network; nothing persists by default (P3).
- The `.tex` is rendered once, by core, and compiled as-is everywhere (P1).
- `web/dist/` is a build artifact, never source; the devcontainer builds wasm
  glue, the host builds PDF assets.

## Where things live

- Architecture decisions: `docs/adr/` (ADR0000 explains the practice).
- Principles and sliders: `docs/principles.md`.
- Roadmap and its architecture mapping: `docs/roadmap.md`.
