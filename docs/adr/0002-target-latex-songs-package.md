# ADR0002 — Target the LaTeX `songs` package for output

## Status

Adopted (the `songs` target remains; the initial header-keyval sketch is
superseded by ADR0011)

## Decision

Generated output will target the LaTeX [`songs` package]: songs are emitted as
`\beginsong{...}[by={...}, key={...}, capo={...}]` blocks containing
`\beginverse` / `\beginchorus` environments, with chords rendered inline as
`\[Chord]` macros so they typeset above the lyrics.

## Context

"LaTeX songbook format" is not one format; several packages and conventions
exist. The choice of target determines the renderer's entire output shape and
what the user can do with the result (e.g. chord diagrams, transposition,
indexes — all built into `songs`).

## Options considered

- **`songs` package (adopted)** — the modern de-facto standard for LaTeX
  songbooks; native verse/chorus structure, chord-over-lyrics rendering,
  song indexes, transposition support; well documented
- **`songbook` package** — older, less actively maintained; different chord
  convention (`[\bf...]`); fewer built-in conveniences
- **Custom/simple template** — smallest implementation surface and no
  package dependency, but we would re-invent verse/chorus handling and lose
  transposition/indexing for free

## Consequences

- The renderer is coupled to `songs` syntax; a different target later means a
  new renderer module, but the parser stays reusable
- Chord placement is position-sensitive (`\[Chord]` goes immediately before
  the character the chord lands on) — the renderer must preserve column
  alignment semantics from the inline `[Chord]` input markers
- The `songs` package must be fetchable by our LaTeX toolchain (verified with
  Tectonic, see ADR0007)

[songs package]: https://ctan.org/pkg/songs

## Relevant principles

None yet.

## Relevant radar blips

LaTeX `songs` package: adopt.

## Advice

- `@nikolaioellegaard` (project owner), 2026-11-01: selected the `songs`
  package as the output target when offered songs/songbook/custom options;
  agreed with the recommendation on the basis that it is the most common
  choice for modern songbooks.
