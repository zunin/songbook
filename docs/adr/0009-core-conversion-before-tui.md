# ADR0009 — Build the conversion core before the TUI

## Status

Adopted (all milestones complete; batch scope was confirmed by ADR0013)

## Decision

Implementation proceeds milestone-by-milestone in this order:

1. ~~Scaffold: Cargo project, devcontainer, CLI skeleton, first fixture~~
   (complete)
2. ~~Parser: metadata extraction, two-line chord format, classifier
   hardening, golden tests~~ (complete; golden tests deferred per ADR0008)
3. ~~Renderer completion: chord-only promotion, repeated choruses,
   standalone template checks, tectonic verification over all outputs~~
   (complete)
4. ~~TUI triage screen (ratatui)~~ (complete)
5. ~~TUI preview + export screen~~ (complete)
6. ~~Polish: `--yes` audit report, error handling, clippy/fmt; batch-directory
   mode~~ (complete; batch mode was promoted from stretch goal to in-scope by
   the advisory board, recorded in ADR0013)

The core conversion (2–3) was built and verified before any TUI work (4–5),
as intended. All milestones are complete as of 2026-11-01.

## Context

The conversion quality — how well the parser handles real UG noise and how
faithful the `songs` output is — is the project's core value and is testable
without any UI. Building the TUI first would couple review UX to an unstable
data model.

## Options considered

- **Core first, TUI after (adopted)** — stable `Song` model before building
  screens on top of it
- **TUI first** — earlier visible feedback for the owner, but every parser
  change would churn UI code; deferred
- **Everything in parallel** — no meaningful benefit at this scale, higher
  rework risk

## Consequences

- The owner sees usable `.tex` output early (already demonstrated in the
  scaffold), before any TUI exists
- `--yes` mode must exist from the start (it already does) so the TUI is an
  enhancement, not a requirement
- Batch-directory mode was initially treated as a stretch goal; its priority was
  later confirmed by the advisory board, and ADR0013 records its promotion into
  milestone 6 and completion
- Real UG paste fixtures from the owner are awaited; constructed fixtures
  cover the gap until then

## Relevant principles

None yet.

## Relevant radar blips

None yet.

## Advice

- Assistant (working session), 2026-11-01: proposed the sequencing; project
  owner approved proceeding with the scaffold on this basis (milestone 1
  completed 2026-11-01).

## Errata

- Assistant (facilitator), 2026-11-01: the original Consequences said,
  "Batch-directory mode stays a stretch goal; its priority has not been
  confirmed (open question)." The board later promoted batch mode into
  milestone 6; ADR0013 records that decision. The consequence is updated to
  preserve this chronology.
