# Roadmap

Priorities set by the project owner (feature decider) on 2026-11-01, mapped
to architecture needs by the facilitator. Order is the board's selection
order; cheap work may interleave between threshold tracks.

| # | Direction | Architecture impact | Process need |
|---|---|---|---|
| 1 | **Songbooks (multi-song)** — the product's namesake: a book of songs from many files, with the `songs` package's song index and table of contents | **Phase 1 shipped 2026-11-01** (core `book` module + CLI `--book`, ADR0018). Phase 2 (web) has one engine work item: book PDFs need the `songs` package's three-step index pipeline (`songidx.lua`) or a pre-seeded `.sbx` — part of the Phase 2 option check | Phase 2 wasm/web changes come back to the forum as their own option paper |
| 2 | **Paste input** (web) — revisits the files-only boundary | Touches ADR0005/ADR0016 boundaries | **Threshold**: privacy analysis + board advice before implementation |
| 3 | **Transposition** and chord tools | Core feature work; no architecture change expected | Below threshold: implementer decides and records |
| 4 | **PDF options** — paper sizes, fonts, per-song options via the `songs` package | Engine/pack parameters and UI; pack changes bump pins deliberately | Mostly below threshold; pack content changes go through the pin |

Working agreement: threshold items open with an option paper and the board's
advice (veto applies); everything below is decided by the implementer and
recorded in `docs/adr/` when it deserves a record.
