# ADR0007 — Use Tectonic for LaTeX compilation verification

## Status

Adopted (Tectonic is the default; no TeX Live fallback script is present)

## Decision

Generated LaTeX is verified by compiling it with Tectonic (`tectonic
-X compile`), with the Tectonic binary installed directly in the devcontainer
image. A full TeX Live container (`texlive/texlive`) was considered as an
optional fallback for fidelity edge cases; no fallback script is present.

## Context

We need to prove generated `.tex` files actually compile, ideally in the same
fast inner loop as code changes. The host has no LaTeX; Docker is available.
The `songs` package must be resolvable by whatever toolchain we pick.

## Options considered

- **Tectonic in the devcontainer (adopted)** — a single ~50 MB static binary;
  auto-fetches packages (including `songs`) from CTAN on first use and caches
  them; verification is just `tectonic out.tex` with no second container or
  docker-socket plumbing
- **Full TeX Live container** (`texlive/texlive`, ~4 GB) — highest fidelity,
  but pulls 4 GB and adds a container round-trip to every iteration
- **No compile verification** — renderer tests only; risks shipping `.tex`
  that does not compile (chord placement, escaping, and `songs` syntax are
  exactly where silent breakage happens)

## Consequences

- Tectonic uses the XeTeX engine rather than pdfLaTeX; the `songs` package
  works under it (verified: `examples/let-it-be.txt` → `.tex` → 10 KB PDF).
  TeX Live remains an option for future fidelity edge cases; no fallback
  script is currently implemented.
- First compile warms the CTAN cache (~few MB, one time per environment)
- Bundle downloads depend on network availability at first use

## Relevant principles

None yet.

## Relevant radar blips

Tectonic: adopt. Full TeX Live in CI: hold (fallback only).

## Advice

- `@nikolaioellegaard` (project owner), 2026-11-01: reported LaTeX was
  unavailable locally but usable via Docker; chose tectonic-in-devcontainer
   over combining full TeX Live into the devcontainer image after the size
   trade-off (~4 GB) was presented.

## Errata

- Assistant (facilitator), 2026-11-01: repository inspection found no TeX Live
  fallback script. The original Decision said, "A full TeX Live container
  (`texlive/texlive`) is kept as an optional fallback script for fidelity edge
  cases, but is not part of the default loop." The original Consequences said
  "a divergence-specific fallback exists in the TeX Live option." The
  container was considered as a possible future fallback only. The wording
  above is corrected as a factual erratum; the owner approved this errata
  process, and no implementation decision changed.
