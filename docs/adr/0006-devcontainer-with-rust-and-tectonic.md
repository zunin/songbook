# ADR0006 — Develop in a VS Code devcontainer (Rust toolchain + Tectonic)

## Status

Adopted

## Decision

Development happens inside a VS Code devcontainer defined by
`.devcontainer/devcontainer.json` + `.devcontainer/Dockerfile`:

- Base image: `mcr.microsoft.com/devcontainers/rust:1-1-bookworm`
- Tectonic (musl static binary) installed into `/usr/local/bin` at image build
  time, so LaTeX compilation verification works inside the container with no
  host toolchain
- VS Code extensions: rust-analyzer, CodeLLDB, even-better-toml
- `postCreateCommand`: `cargo build` + `tectonic --version` warm-up

## Context

The host machine has Docker but no Rust toolchain and no LaTeX installation.
A LaTeX toolchain is required to verify generated output compiles
(see ADR0007); Rust is required to build the tool itself
(see ADR0003). Options for provisioning both were considered.

## Options considered

- **Devcontainer (adopted)** — reproducible, editor-integrated
  (rust-analyzer, debugging), one command to enter; supports adding tools to
  the image declaratively
- **Install rustup on the host** — smooth iteration, but the host stays
  unpolluted is lost as a benefit, and LaTeX would still need separate
  provisioning
- **Docker run-per-command** (`docker run -v $PWD:/app rust cargo test`) —
  works for CI-style checks but no editor integration, and containers running
  as root leave root-owned `target/` artifacts on the host (observed during
  scaffold verification)

## Consequences

- All Rust and LaTeX verification runs inside the container; the host needs
  only Docker + VS Code
- Building from *outside* the container should be avoided (root-owned
  artifacts); cleanup requires a privileged `rm -rf target`
- Image rebuild needed when tectonic version is bumped (versioned via build
  arg)

## Relevant principles

None yet.

## Relevant radar blips

Dev containers: adopt.

## Advice

- `@nikolaioellegaard` (project owner), 2026-11-01: proposed devcontainers
  for the Rust toolchain after the initial plan assumed local installation;
  confirmed tectonic-in-devcontainer over a Rust-only container plus an
  external TeX Live container.
