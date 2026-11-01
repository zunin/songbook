# ADR0008 — Golden-file tests for parser and renderer

## Status

Deferred (revisit when real fixtures exist)

## Decision

*(Proposed)* Parser and renderer behavior is pinned by golden files: each
`examples/*.txt` fixture has a checked-in expected `.tex` output, and tests
fail on drift. Regenerating goldens is an explicit, auditable act (a "bless"
mechanism) rather than a silent update.

## Context

The classifier is heuristic and will keep being tuned as real Ultimate Guitar
pastes are added. Both under-classification (noise kept) and
over-classification (lyrics dropped) are silent failures that unit tests on
individual rules do not catch end-to-end. Real fixture inputs are expected
from the project owner.

## Options considered

- **Plain checked-in golden files + bless flag (proposed)** — transparent,
  diffable in review, no extra dependency; blessing is a deliberate commit
- **Snapshot library (`insta`)** — nicer review UX for many snapshots, but
  adds a dependency for a small fixture set
- **Unit tests only** — good for rules (chord regex, header detection) but
  blind to end-to-end output drift; keep unit tests *and* add goldens

## Consequences

- Every classifier/renderer improvement requires a conscious decision to
  re-bless outputs, making behavior changes visible
- Fixture files must be curated to cover: inline-chord lyrics, section
  headers, chord-only lines with repeats, fretboard lines, noise blocks, and
  ambiguous lines
- Compile verification (ADR0007) can loop over all golden outputs, tying the
  two verification mechanisms together

## Relevant principles

None yet.

## Relevant radar blips

None yet.

## Advice

- Assistant (working session), 2026-11-01: recommended plain golden files
  over `insta` for a project this size. Project owner has not yet confirmed;
  status will move to Adopted on confirmation.
- Project owner (advisory board), 2026-11-01: after hearing the trade-offs,
  advised **deferring** — unit-level tests are sufficient for now; the
  end-to-end net is worth adopting once several real messy fixtures exist
  (the owner has supplied one so far). Accepted: unit tests only during
  milestone 2; this ADR returns to Proposed when the fixture library grows.
  The harness is small enough that late adoption costs nothing extra.
