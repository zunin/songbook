# ADR0003 — Implement the CLI in Rust

## Status

Adopted

## Decision

`songbook` will be implemented in Rust.

## Context

The tool is text-processing-heavy (parsing noisy input) with an interactive
TUI component. Language choice affects ecosystem fit for both, and the
distribution story for a personal CLI.

## Options considered

- **Rust (adopted)** — `ratatui`/`crossterm` are the strongest current TUI
  stack; `clap` gives a polished CLI; compiles to a single static binary;
  strong typing helps model the parser state machine
- **Go** — `bubbletea`/`lipgloss` TUI ecosystem is also capable; simpler
  language; static binaries via goreleaser
- **Python** — fastest to iterate on text parsing; weakest TUI story;
  distribution via pipx/uv is fine but runtime-dependent

## Consequences

- Slower initial iteration than Python, mitigated by the small scope
- TUI work lands on `ratatui` (see ADR0004)
- Distribution can eventually be a single binary via cargo/brew
- Builds happen inside the devcontainer (see ADR0006); note that docker-run
  builds from outside the container leave root-owned `target/` artifacts —
  prefer building inside the container

## Relevant principles

None yet.

## Relevant radar blips

Rust: adopt. ratatui: adopt.

## Advice

- `@nikolaioellegaard` (project owner), 2026-11-01: narrowed the choice to
  Rust or Go (expressing no strong preference between them), then accepted
  the Rust recommendation driven by the ratatui ecosystem.
