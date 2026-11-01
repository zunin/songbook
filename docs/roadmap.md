# Roadmap

Priorities set by the project owner (feature decider) on 2026-11-01, mapped
to architecture needs by the facilitator. Order is the board's selection
order; cheap work may interleave between threshold tracks.

| # | Direction | Architecture impact | Process need |
|---|---|---|---|
| 1 | **Songbooks (multi-song)** — the product's namesake: a book of songs from many files, with the `songs` package's song index and table of contents | New core multi-song API; batch and web surfaces grow a second output shape; on-disk/book organization decisions | **Threshold**: option paper at the next forum before implementation (public API + possibly file-format decisions) |
| 2 | **Paste input** (web) — revisits the files-only boundary | Touches ADR0005/ADR0016 boundaries | **Threshold**: privacy analysis + board advice before implementation |
| 3 | **Transposition** and chord tools | Core feature work; no architecture change expected | Below threshold: implementer decides and records |
| 4 | **PDF options** — paper sizes, fonts, per-song options via the `songs` package | Engine/pack parameters and UI; pack changes bump pins deliberately | Mostly below threshold; pack content changes go through the pin |

Working agreement: threshold items open with an option paper and the board's
advice (veto applies); everything below is decided by the implementer and
recorded in `docs/adr/` when it deserves a record.
