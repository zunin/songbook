# ADR0014 — Triage over every raw input line

## Status

Adopted

## Decision

The TUI triage review unit is **every raw input line**, not only the lines the
classifier flagged.

1. The triage screen shows the full raw input (as it already does for context)
   and `j`/`k`/arrows move over **all** lines. The current line can be kept,
   dropped, reclassified (lyric / chord-only / section header / noise), or
   edited, whether or not the classifier flagged it.
2. Classifier flags (`Ambiguous`, `Fretboard`) still mark lines as **pending**
   and `a` still accepts the remaining pending flags; flags are guidance, not
   the boundary of what can be reviewed.
3. Default decisions follow `parse` plus the established interactive defaults:
   - preamble lines (metadata/noise consumed by `analyze_preamble`) default to
     "not in the song body"; metadata stays extracted into the metadata panel
   - a chord-only line immediately above a lyric merges into it by default
   - blank/noise lines are dropped by default
   - section headers start sections
   - ambiguous lines default to lyric (the existing ADR0012 default)
   - fretboard lines are kept but unrendered (current renderer behavior)
4. The final `Song` is rebuilt from the raw lines plus their decisions at
   triage finish, then rendered. Metadata panel values override inferred ones.
5. Fitness check: finishing triage without manual changes must produce the same
   output as `parse` followed by the interactive defaults: ambiguous lines are
   kept as lyrics, while blank/noise lines remain excluded. A test pins this
   behavior for representative inputs and must cover whitespace and chord-line
   merging to guard the raw-line rebuild against parser drift.

## Context

Users reviewing the full input (added for context) can see lines the classifier
confidently misclassified — e.g. a lyric dropped as noise, or a junk line kept
as lyric — but those lines were unreachable because only flagged lines were in
the review queue. The project owner (advisory board) asked for the ability to
edit any line when the classifier is wrong, and chose full raw-line review over
recovering only dropped lines.

## Options considered

- **Review every raw line (adopted)** — maximum honesty: what you see is what
  you can fix; requires rebuilding from raw lines and an equivalence test.
- **Recover dropped lines only** — smaller change, but confidently-kept wrong
  lines (e.g. an ad read as a lyric) stay unfixable.
- **Flagged lines only (previous)** — smallest, but hides the most common
  false-negative class.

## Consequences

- `TriageState`'s queue becomes a flag set over raw indices; `items` (flattened
  `Song`) is no longer the source of truth. Existing state tests are refactored
  to the raw-line model.
- The rebuild uses `preamble_len` for the parser's preamble boundary and
  reproduces its body classification/chord-merging behavior; parity tests must
  be updated when those parser rules change.
- Preamble lines are visible in review; editing them cannot change extracted
  metadata (the metadata panel does that), but they can be restored to the song
  body if they were wrongly consumed.
- Undo snapshots cover per-line decisions instead of parsed items.

## Relevant principles

None yet.

## Relevant radar blips

None yet.

## Advice

- Project owner (advisory board), 2026-11-01: asked for editing whenever the
  classifier produced a false positive; chose "any line in the input" over the
  narrower alternatives. Assistant recorded this decision and the equivalence
  fitness check.

## Errata

- Project owner (advisory board), 2026-11-01: clarified that the no-edit
  baseline means `parse` followed by the interactive defaults, not literal
  parser output; in particular, an ambiguous line defaults to lyric under
  ADR0013. The original wording said "same as parse" and was too broad.
- Assistant (facilitator), 2026-11-01: corrected the Consequences description.
  Its original wording said triage "must duplicate `parse`'s merge/preamble
  rules"; the implementation uses the shared `preamble_len` helper and
  reproduces body classification/merge behavior.
