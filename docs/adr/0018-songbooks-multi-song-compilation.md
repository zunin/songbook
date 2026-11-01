# ADR0018 — Songbooks (multi-song compilation)

## Status

Adopted (2026-11-01)

## Decision

Songbook gains multi-song book compilation through a new core API,
`songbook-core::book`. A book is one LaTeX document: the existing per-song
renderer produces each `\beginsong…\endsong` body unchanged (P1), and the book
renderer composes them into a `songs` environment with an optional title page
and the package's indexes. The document is always standalone.

- **Membership and metadata come from a plain-text manifest** (v1 format, no
  new dependency — P5): UTF-8 text; `#` comments and blank lines ignored;
  optional `Title: …` and `Author: …` header lines before the first path; then
  exactly one `.txt`/`.crd` path per line, resolved relative to the manifest's
  directory. Unknown `Key: value` header lines are errors (typo protection).
  Absolute paths are discouraged.
- **Order is the manifest's order as written**; the documented convention is
  to list songs in path order. There is no implicit discovery: every song must
  be listed.
- **Indexes**: the title index is included by default; the chord index is
  opt-in. Placement and required package options follow the pinned `songs`
  package documentation, verified by compilation — not guessed.
- **Output**: `-o/--out` names the `.tex`; by default it is the manifest's
  stem plus `.tex`, beside the manifest.
- **Surface and phasing**: CLI first — `songbook --book <manifest>` with book
  mode non-interactive (non-interactive defaults per song, per-song audit
  summary; unreadable or invalid songs are skipped and reported, exit nonzero
  if any were skipped, mirroring batch). Web second (Phase 2): multi-file
  selection, sequential triage, book `.tex` and PDF export through the
  existing engine. Phase 1 does not change the wasm crate or the web app.
- **Fitness functions**: a golden book fixture (byte-exact) per ADR0008 and a
  multi-song PDF smoke check. Where native Tectonic is unavailable (host), the
  smoke asserts compile status, page count, and all titles/lyrics in extracted
  text; the native byte-exact reference remains the devcontainer check.

## Context

ADR0002 makes `songs`-package LaTeX the product format, and the package is
designed for books: many songs in one environment, with generated indexes.
Today the renderer emits one song (ADR0009, ADR0011) and batch writes one
`.tex` per input file; the web app is single-song (ADR0016). The PDF engine
(ADR0017) compiles arbitrary documents, so a book is a document-level concern
above the song level.

Alternatives rejected:

- **Thin assembler outside core** (batch or JS concatenation): book semantics
  (wrapper, numbering, indexes) would drift out of core (P2).
- **CLI-only extension**: the web must reuse the book API in Phase 2, so it
  belongs in core.
- **Directory-as-book**: the board chose explicit membership over implicit
  discovery; a directory's contents are not a stable definition of a book.

## Consequences

- New public core API (manifest model + book renderer); the wasm layer grows a
  book export only in Phase 2.
- P1 holds: book rendering calls the single song renderer and never
  re-implements song LaTeX.
- Manifest files become a new versioned on-disk input; they travel with the
  songs they list and should avoid absolute paths.
- Book mode is non-interactive in Phase 1; interactive book triage is web
  Phase 2.
- No new dependencies: manifest parsing is in-repo.

## Advice

- **Advisory board** (project owner), 2026-11-01, forum session three: chose
  core module design A; chose a **manifest file** over directory mode; chose
  path order with the title index by default; chose CLI first, web second. No
  veto.
- The facilitator had recommended directory mode; the board overruled in
  favour of explicit membership. The manifest's plain-text syntax follows P5
  and is stated here for veto.

## Limitations (accepted)

- No per-book chord consistency or transposition (roadmap item 3).
- No manifest scaffolding or generation command in v1.
- Index selection is limited to title (default) and chords (opt-in); no
  custom or scripture indexes in v1.
- No directory discovery; membership is exactly what the manifest lists.
- Book mode has no interactive triage until web Phase 2.
