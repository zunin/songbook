# ADR0004 — Heuristic line classification with TUI line-by-line triage

## Status

Adopted (classifier and TUI implemented; triage scope is refined by ADR0014;
non-interactive behavior by ADR0013 and audit scope by ADR0015)

## Decision

Parsing runs in two stages:

1. **Heuristic classification.** The parser classifies every input line:
   section header (`[Verse 1]`, `[Chorus]`, …), lyric with inline `[Chord]`
   markers, chord-only line, fretboard tab line, website noise (credits,
   tuning, ads, URLs), blank, or ambiguous. Chord tokens are validated
   against a chord-name grammar so structure markers are not mistaken for
   chords.
2. **Interactive triage.** A TUI (ratatui) walks the user through every line
   the classifier flagged or marked ambiguous: keep, discard, reclassify
   (section header / chord-only / lyric), edit, undo, or auto-accept the rest.
   A preview screen shows the final song structure before export.

A `--yes` flag skips the TUI and trusts the heuristics.

## Context

Ultimate Guitar pastes are reliably noisy and inconsistently formatted across
tabs and time. Pure automation would silently misclassify lines; pure manual
cleanup defeats the purpose of the tool. The owner explicitly wanted a review
step where comments can be ignored and chord parsing verified.

## Options considered

- **Heuristics + line-by-line triage (adopted)** — best effort/review balance;
  the user confirms only what the machine is unsure about
- **Full auto with heuristics only** — fast but silently wrong on edge cases;
  kept available as `--yes` for non-interactive use
- **Two-pane editor (original left, cleaned right)** — most flexible, but a
  significantly larger TUI build for v1; revisit if triage proves insufficient
- **Auto + whole-document accept/reject** — too coarse: one bad classification
  forces a full reject

## Consequences

- Parser and renderer share the `Song`/`Section`/`LineKind` model; the TUI
  reviews raw input lines and rebuilds a `Song` for rendering (ADR0014)
- The classifier must expose an explicit `Ambiguous` state rather than
  guessing, so triage has a well-defined queue
- `--yes` needs an audit trail (a printed summary of what was auto-discarded)
  to stay trustworthy
- TUI dependency (ratatui + crossterm) is accepted (see ADR0003)

## Relevant principles

None yet.

## Relevant radar blips

ratatui: adopt. Regex-based text classification: adopt (deliberately not ML —
rules are debuggable and the domain is narrow).

## Advice

- `@nikolaioellegaard` (project owner), 2026-11-01: chose line-by-line triage
  over the two-pane editor and auto-accept/reject alternatives; chose
  file-based input (see ADR0005), which makes deterministic classification
  and testing feasible.

## Errata

- Assistant (facilitator), 2026-11-01: ADR0014 changed the triage model. The
  original Consequences said, "Parser and TUI share one song model
  (`Song`/`Section`/`LineKind`); triage edits must mutate that shared
  structure". The TUI now reviews raw lines and rebuilds `Song`; the parser
  and renderer still share the model.
