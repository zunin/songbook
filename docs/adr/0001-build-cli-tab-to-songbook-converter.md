# ADR0001 — Build a CLI that converts ASCII tabs to LaTeX songbook format

## Status

Adopted

## Decision

We will build `songbook`: a command-line tool that converts ASCII tab text in
the chord-over-lyrics style used by sites such as Ultimate Guitar and Guitaretab
into LaTeX songbook documents.

## Context

Building a personal songbook from online tab sources is currently manual and
tedious: pasted tabs are full of website noise (credits, ads, tuning notes,
chord diagrams), and hand-converting them to LaTeX is slow and error-prone.
Automating the conversion requires deciding, for every line of noisy input,
whether it is lyric, chord, structure, or junk — a judgment call that pure
automation will sometimes get wrong.

## Options considered

- **CLI converter tool (adopted)** — repeatable, scriptable, fits a
  developer's workflow; the noisy-input problem is addressed by a review step
- **Manual conversion** — no tooling investment, but slow, inconsistent, and
  does not scale to a full songbook
- **Existing converters (chordpro → LaTeX etc.)** — existing tools target
  chordpro-format input, not UG-style pastes with their noise; adapting them
  would be as much work as building the parser, with less control
- **Fully automated pipeline (scrape URL → PDF)** — most convenient in
  principle, but scraping is fragile and removes human control over what is
  included

## Consequences

- We need a line classifier robust enough to be useful, with an escape hatch
  for ambiguity (see ADR0004)
- The tool is text-in/text-out; no network access, no scraping
  (see ADR0005)
- Scope for v1: single songs, parsed one at a time; whole-book assembly is a
  later concern

## Relevant principles

None yet (project has no principles file; revisit as the project grows).

## Relevant radar blips

None yet.

## Advice

- `@nikolaioellegaard` (project owner), 2026-11-01: initiated the project with
  the goal of converting ASCII tab format to LaTeX songbook format; scoped v1
  around parsing individual songs with an interactive review step so that
  comments/noise can be reliably ignored and chord parsing can be verified.
