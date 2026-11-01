# ADR0010 — Metadata inference and selective tuning rendering

## Status

Adopted

## Decision

1. **Title/artist inference**: when a paste has no `Song:`/`Title:` /
   `Artist:` labels, the parser infers title and artist from the first
   bare (non-classified) preamble lines: first line is title, next is
   artist. The risk of silently consuming a lyric line is accepted, because
   the TUI triage screen (ADR0004, milestone 4) lets the user reclassify and
   correct metadata before export.
2. **Tuning rendering**: the parsed `Tuning` field is rendered only when it
   differs from standard tuning (E A D G B E). Standard (and absent) tuning
   produces no output. Non-standard tuning is shown in the song header area.

## Context

After milestone 2 (parser hardening), two open questions were put to the
project owner (the advisory board):

- Whether unlabelled bare first/second lines should be inferred as
  title/artist. Unlabelled pastes exist; the parser previously left them as
  "Untitled".
- Whether the parsed-but-unused `Tuning` field should stay parser-only or be
  rendered. Tuning matters for a songbook only when it deviates from
  standard; rendering standard tuning everywhere would be noise.

## Options considered

- Title/artist:
  - **Labels only** — safe, but unlabelled pastes stay "Untitled" with no
    recovery until manual editing
  - **Infer + TUI correction (adopted)** — best coverage; silent-error risk
    is bounded by the triage screen that follows in milestone 4
  - **Infer, no correction path** — rejected: silent misattribution
- Tuning:
  - **Parser-only** — zero risk, loses information
  - **Render as LaTeX comment** — invisible in output; little value
  - **Render visibly when non-standard (adopted)** — information appears
    exactly when it matters; standard stays clean

## Consequences

- Parser inference adds a class of "best guess" metadata that must be
  surfaced in the TUI triage as correctable (a dependency for milestone 4)
- Renderer needs a tuning field check and a header emission path; the
  "standard tuning" comparison must treat case/spacing tolerantly
- Inference only applies when labels are absent, so labelled pastes are
  unchanged

## Relevant principles

None yet.

## Relevant radar blips

None yet.

## Advice

- Development agent (milestone 2), 2026-11-01: raised both questions;
  recommended labels-only for safety.
- Project owner (advisory board), 2026-11-01: chose inference with TUI
  correction, and rendering tuning when different from standard.
