# ADR0000 — Record architecture decisions in this repository

## Status

Adopted

## Decision

We will record all architecturally-significant decisions for `songbook` as
lightweight Architecture Decision Records (ADRs) in `docs/adr/`, following the
process described in *Facilitating Software Architecture* (Harmel-Law et al.):
decisions are made by whoever has the need and accountability, using the
**Advice Process**, and captured here.

The Advice Process, applied to this project (a small, primarily single-maintainer
project):

- **The rule:** anyone working on `songbook` can make an architectural decision.
- **The qualifier:** before deciding, the decision-taker must consult (a) everyone
  meaningfully affected by the decision, and (b) people/expertise in the relevant
  area. Advice must be sought, listened to, and recorded — but need not be
  followed. No consensus is required.
- Concretely here: the project owner (`@nikolaioellegaard`) is the primary
  affected party; advice comes from conversations with the project owner,
  domain research (e.g. the LaTeX `songs` package documentation, TUI framework
  ecosystems), and practical experimentation (e.g. compile checks).

Each ADR uses these sections (per the author's own format):

- **Title** — unique identifier and the decision itself
- **Status** — Draft / Proposed / Adopted / Deferred / Partially superseded /
  Superseded / Retired
- **Decision** — the decision in a few sentences
- **Context** — the forces and circumstances necessitating the decision
- **Options considered** — each option briefly, with pros/cons; the adopted
  option first
- **Consequences** — ramifications, positive and negative
- **Relevant principles / radar blips** — alignment with (or divergence from)
  team principles and the technology landscape; "none yet" is acceptable early on
- **Advice** — the raw record of advice received: giver, date, and what was said,
  including advice that was *not* followed

ADRs are point-in-time decision records. A change to an adopted decision
produces a new ADR that supersedes the old one; status and supersession
references may be updated in the old record.

Factual or editorial corrections that do not change a decision may be made
in place only with a dated `Errata` section recording the original wording,
the correction, and its reason. The original wording must remain visible in
the erratum. Corrections must not silently rewrite the decision history.

## Consequences

- Decision context survives beyond the conversation that produced it
- Later decisions can reference and supersede earlier ones cleanly
- The format doubles as a thinking checklist before deciding
- Slight overhead per decision; acceptable at this project's scale

## Advice

- Project owner (advisory board), 2026-11-01: approved explicit, dated errata
  for factual corrections that do not change an adopted decision; substantive
  decision changes still require a new superseding ADR.

## Errata

- Project owner (advisory board), 2026-11-01: the original wording said,
  "the old record is edited only to update its status." This rule was too
  strict for factual corrections that do not change a decision; the dated
  errata process above is now permitted. The advice-process and supersession
  decisions are unchanged.
