# Songbook guiding principles

Adopted by the project owner (advisory board) on 2026-11-01, drafted by the
architecture facilitator from the ADR corpus (docs/adr). Principles are few
on purpose: they encode trade-offs, not rules. An ADR may argue against a
principle, never silently around one. Moving a slider is a threshold
decision and goes through the advice process (see P7).

## Principles

### P1 — One rendering of truth

The generated `.tex` is the single artifact. Verification compiles exactly
what the user gets; no component ever re-implements the output format.
(Evidence: ADR0007, ADR0017.)

### P2 — Core owns semantics; UIs are thin views

Parsing, triage, and rendering semantics live in `songbook-core`. The CLI/TUI
and the browser app drive the same core; they never duplicate its decisions.
(Evidence: ADR0009, ADR0016.)

### P3 — Privacy by architecture

No server, no upload of song content, no persistence by default. Every new
capability must preserve this boundary or come to the forum with a reason.
(Evidence: ADR0016, ADR0017.)

### P4 — Reproducibility over convenience

Pinned hashes, byte-verified generation, deterministic builds. A deliberate
change bumps the pin; an accidental one fails the build. (Evidence: ADR0007,
ADR0008, ADR0017.)

### P5 — Boring, dependency-light surfaces

Vanilla web, a small dependency set, no frontend framework. Every new
dependency is a threshold decision. (Evidence: ADR0016.)

### P6 — Conservative licensing, carried sources

Assume the most restrictive plausible license. Nothing ships without its
notices and complete corresponding sources. (Evidence: ADR0017,
THIRD_PARTY_NOTICES.md.)

### P7 — Decisions get advice and a record

Threshold decisions (new dependencies, new components, public APIs, file
formats, cross-cutting patterns — anything hard to reverse or felt in more
than one place) seek the advisory board's advice before implementation and
land in `docs/adr/` with the advice recorded. Everything below the threshold
is decided by the implementer and still recorded. The board holds an
absolute veto at the threshold. (Established: this forum, 2026-11-01.)

## Trade-off sliders

Current positions of the board. They are defaults for arguments, not laws.

| Slider | Position | Notes |
|---|---|---|
| Output fidelity ⟷ implementation simplicity | firmly toward fidelity | the `songs`-package output is the product |
| Privacy ⟷ capability reach | firmly toward privacy | client-only won over server-side PDF |
| Reproducibility ⟷ iteration speed | toward reproducibility | |
| Few dependencies ⟷ feature velocity | toward few dependencies | |
| Human oversight ⟷ automation | toward human oversight | except where a fitness function verifies automatically (golden files, smoke test, build pins) |

## Advice record

- 2026-11-01, architecture advisory forum (session two): the board adopted
  principles P1–P7 and the slider positions as drafted, without amendment
  and without veto.
