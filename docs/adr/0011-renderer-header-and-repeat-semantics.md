# ADR0011 — Renderer header, chord-only, and repeated-chorus semantics

## Status

Adopted

## Decision

1. **Song header metadata** is emitted with macros the `songs` package
   actually defines. `by={...}` remains a `\beginsong` keyval; key and
   non-standard tuning are emitted as `\textnote{Key: ...}` /
   `\textnote{Tuning: ...}` immediately after `\beginsong`; a numeric capo is
   emitted with the package's `\capo{n}` macro (falling back to a
   `\textnote` for non-numeric values). The `key=` and `capo=` keyvals are
   **not** used because the package rejects them as undefined keys.
2. **Chord-only lines** are rendered as bare inline chord macros
   (`\[Am] \[F] ...`) in the section where they occur. Repeat tokens such as
   `2x` are emitted as a following `\textnote{2x}` because they are not
   chords. A chord-only line immediately above a lyric line continues to be
   merged into that lyric by the parser.
3. **Repeated choruses**: consecutive chorus sections whose lyrics are equal
   after ignoring case, whitespace, and punctuation are collapsed into the
   first chorus plus a `\textnote{Chorus repeats}` marker. Choruses whose
   words differ are kept separately. The package has no per-song
   `\repchorus`; `\repchoruses` is a page/column auto-insertion feature and
   is not a substitute for deduplicating source sections.

## Context

Milestone 3 required the renderer to emit all parsed constructs and to
compile against the real `songs` package under Tectonic. Compiling the first
metadata-bearing output exposed that the previously assumed `key=` and
`capo=` `\beginsong` keyvals are undefined in the package (only `by`, `sr`,
`cr`, `li`, `index`, and `ititle` are built in). The package does provide
`\capo`, `\textnote`, and `\musicnote` for header-style notes.

The package documents chord replaying (verses and `\memorize`/`\replay`) and
automatic page-level chorus repetition (`\repchoruses`), but no macro for
marking a specific chorus as a repeat of the previous one. Duplicating a
chorus body verbatim is valid LaTeX but noisy; silently dropping a
*different* chorus loses lyrics.

## Options considered

- **Supported macros for header notes (adopted)** — compiles with the
  package as published and remains valid in non-standalone output embedded
  in a larger songbook; key/tuning appear as visible notes rather than
  structured metadata.
- **Custom `\newsongkey` fields in our preamble** — keeps metadata
  structured, but only works for standalone output; embedded output would
  require the host document to duplicate our preamble definitions.
- **Keep `key=`/`capo=` keyvals** — rejected: package errors immediately
  (`Package keyval Error: key undefined`).
- **Repeated choruses: collapse equivalent bodies + textual note (adopted)**
  — removes verbatim duplication while preserving the fact of repetition.
- **Repeated choruses: use `\repchoruses`** — rejected: it controls
  page-builder insertion on later columns/pages, not conversion of duplicate
  source sections.
- **Repeated choruses: keep every duplicate verbatim** — safe but noisy;
  remains the fallback when the words differ.

## Consequences

- ADR0002's sketch of `\beginsong[... key=..., capo=...]` is corrected in
  practice; key and tuning are visible notes, not transposition metadata.
- `\textnote` is visible in both lyric and chorded books; `\capo` follows
  the package's music-note behavior.
- Chorus equivalence is deliberately conservative: only case, whitespace,
  and punctuation differences are ignored. Word changes produce two
  choruses, so no lyric content is lost.
- The repeat marker is a `\textnote`, so downstream consumers see the same
  package-native structure as other musician notes.

## Relevant principles

None yet.

## Relevant radar blips

`\textnote` / `\capo`: adopt. `\repchoruses`: hold (not a source-level
deduplication tool).

## Advice

- Development agent (milestone 3), 2026-11-01: inspected the CTAN `songs`
  package source/documentation and Tectonic compilation to confirm the
  available keyvals and macros; recommends the supported-macro approach and
  the conservative chorus equivalence above. Submitted as Proposed for the
  advisory board to confirm or amend.
- Project owner (advisory board), 2026-11-01: confirmed all three points —
  `\textnote` for key/tuning, collapse + note for identical repeated
  choruses, `\textnote` fallback for non-numeric capo. Adopted as written.
