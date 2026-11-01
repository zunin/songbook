# ADR0019 — Songbooks Phase 2: web books

## Status

Adopted (2026-11-01)

## Decision

The browser app gains book output, completing ADR0018's Phase 2:

- **Book assembly stays in core.** The web triages songs one at a time in the
  existing wasm `Session`s; a minimal new wasm ferry hands the triaged bodies
  and chord data to `songbook-core::book`'s renderer. No LaTeX is ever
  assembled in JavaScript (P1, P2).
- **Web flow**: multi-file selection → sequential per-song triage (the
  existing UI, one song at a time) → book Title/Author → Export book `.tex` /
  Export book PDF. Still no persistence and no upload (P3).
- **Real indexes in the browser.** The `.sxd → .sbx` conversion (normally the
  songs package's `songidx.lua`) is ported into core as `songbook-core::songidx`,
  golden-tested byte-exact against native `songidx.lua` output generated in
  the devcontainer from the pinned songs package. The PDF worker runs
  multi-pass compiles: pass 1 → fetch the `.sxd` from the engine's outputs →
  convert via the port → feed the `.sbx` back → pass 2 (and a final pass when
  the package requires one). The engine already exposes per-file outputs and
  accepts files between compiles, so the engine itself is unchanged.
- **Fitness functions**: committed native golden `.sbx` fixtures for the port;
  the PDF smoke upgraded to compare the browser book's extracted text against
  a natively generated reference (three-step compile in the devcontainer).
- **Payload**: the worker additionally loads the songbook wasm (~1.4 MB)
  alongside the Tectonic engine — accepted within the ADR0017 payload
  budget's spirit; the book path remains lazy-loaded.

## Context

ADR0018 shipped book compilation for the CLI (Phase 1) and deferred web
books. The browser engine (ADR0017) compiles arbitrary documents, but a
single pass cannot run the songs package's three-step index pipeline, so book
index pages showed the package's placeholder. Engine inspection confirms the
pipeline is reachable without engine changes: compile outputs are retrievable
per file, and files can be added between compiles.

Alternatives rejected:

- **JS `.sxd → .sbx` in the worker**: book semantics outside core (P2), drift
  risk against native songidx.
- **Placeholder-only books in the browser**: weakest product; the port is
  modest and verifiable.
- **Lua in the browser** (fengari/wasmoon): a heavy new frontend dependency
  (P5).

## Consequences

- New wasm public API (the ferry) and a second wasm module loaded in the PDF
  worker. Worker-side module loading may interact with the page's CSP
  (Trusted Types); if it does, that is verified and fixed during
  implementation within the same strictness (same-origin only).
- The book PDF smoke gains a native text reference; fixtures with native
  references must pass byte-exact.
- songidx's collation behavior becomes part of this repo's tested surface;
  a pinned songs upgrade must re-verify the port.
- The chord-index injection point (first line after `\beginsong`) stays
  pinned by tests (ADR0018).

## Advice

- **Advisory board** (project owner), 2026-11-01, forum session four: chose
  E1 (core port of the index converter); delegated the exact wasm ferry shape
  to the implementer under the no-LaTeX-in-JS constraint; approved the
  proposed web flow. No veto.
- At the Phase 2 click-test (2026-11-01) the board approved the shipped
  result, in which the book Title/Author form precedes triage (the Decision's
  flow summary listed it after triage; the shipped order stands as accepted).

## Limitations (accepted)

- Index fidelity targets `songidx.lua` at the pinned songs version; other
  versions may sort differently.
- Web book triage is sequential, one song at a time; no batch edit UI in this
  phase.
- Book order in the browser follows file-selection order (the manifest's
  analog); no reordering UI in this phase.
