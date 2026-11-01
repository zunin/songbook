# ADR0005 — File-based input only (no scraping)

## Status

Adopted

## Decision

The tool reads tab text from local files (`.txt`/`.crd`). It does not fetch
from URLs and does not accept interactive paste input in v1.

## Context

Tab text originates on websites (Ultimate Guitar, Guitaretab). Getting it
into the tool could happen three ways: the user saves/pastes text into a
file, pastes directly into the TUI, or the tool scrapes the site itself.

## Options considered

- **Files only (adopted)** — deterministic, testable, zero network code; the
  small extra step of saving a paste is acceptable
- **Paste into the TUI** — one fewer manual step, but complicates the TUI and
  adds no parsing capability
- **Fetch from URL** — most convenient in principle, but scraping is fragile
  (site layout changes, bot defenses, terms of service) and its maintenance
  cost would dominate the project

## Consequences

- The user performs the copy-paste step manually
- No HTTP client dependency; no network error surface
- Testing is straightforward: fixtures are just files (see ADR0008)
- URL input can be added later without disturbing the parser core

## Relevant principles

None yet.

## Relevant radar blips

None yet.

## Advice

- `@nikolaioellegaard` (project owner), 2026-11-01: selected file-based input
  over paste-into-TUI and URL fetching, accepting the manual save step.
