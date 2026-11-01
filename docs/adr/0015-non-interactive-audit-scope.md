# ADR0015 — Scope of the non-interactive audit report

## Status

Adopted

## Decision

The non-interactive audit documents the decisions available from the parsed
`Song` model; it does not claim to be a line-by-line source audit.

- Single-file mode reports inferred title/artist as guesses (without source
  positions), ambiguous lines by count and full text, dropped fretboard lines
  by count and one sample, collapsed choruses by count, and present metadata.
- Batch mode prints a compact per-file line: guessed title/artist values,
  counts of ambiguous lines kept and fretboard lines dropped, and collapsed
  chorus count, or `no guesses needed`.
- Noise and preamble lines already discarded by `parse` are not reported, and
  neither report gives source line numbers.

## Context

ADR0013's original audit decision promised source positions and referred to an
audit of auto-discarded content. The implementation in `src/audit.rs` reports
heuristic decisions represented by `Song`; it does not retain or enumerate
noise/preamble lines discarded by parsing, and its reports contain no source
line positions. The project owner chose to document the current report scope
rather than expand the source model and report now.

## Options considered

- **Document current parsed-model scope (adopted)** — accurate to the current
  implementation, no parser/source-provenance changes; noise already removed
  during parsing remains outside the audit.
- **Expand the audit to source-line granularity** — would require preserving
  line positions and classifications for lines currently discarded during
  parsing; more complete, but expands parser/audit data and output.

## Consequences

- This ADR partially supersedes ADR0013's audit-report decision; its non-
  interactive defaults, batch behavior, and error handling remain adopted.
- The report is transparent about guesses that survive parsing, but cannot
  claim to list every dropped input line.
- A future source-line audit requires a separate decision and retained or
  reconstructed source-line classification/provenance alongside the parsed
  model.

## Relevant principles

None yet.

## Relevant radar blips

None yet.

## Advice

- Assistant (facilitator), 2026-11-01: identified the mismatch between
  ADR0013's promised source positions / discarded-line coverage and `src/audit.rs`.
- Project owner (advisory board), 2026-11-01: chose to document the current
  report scope rather than expand the audit at this time.
