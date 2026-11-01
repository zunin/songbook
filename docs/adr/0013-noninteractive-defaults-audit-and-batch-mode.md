# ADR0013 — Non-interactive defaults, `--yes` audit report, and batch-directory mode

## Status

Adopted (audit-report scope superseded by ADR0015)

## Decision

1. **Defined non-interactive defaults.** Whenever no interactive review runs
   (`--yes`, the non-TTY fallback, or batch mode), ambiguous lines are kept
   as lyrics — the same default verdict the TUI applies (ADR0012) — and
   fretboard lines remain unrendered. Previously `--yes` silently dropped
   ambiguous lines at render time.
2. **Audit report.** Every non-interactive conversion prints a short audit
   after writing output: inferred (vs labelled) title/artist are flagged as
   guesses with their source line position; ambiguous lines kept as lyrics
   are listed with count and full text; fretboard lines dropped are counted
   (with the first line as a sample); repeated choruses collapsed are
   counted; present metadata fields (labelled title, artist, key, capo,
   tuning) are listed with values. The report is a pure text builder over
   `(Song, raw input)` and is unit-tested; it describes the decisions taken
   from the parsed song before the non-interactive defaults are applied.
3. **Batch-directory mode.** A directory input converts every `.txt`/`.crd`
   file (extension match is case-insensitive) directly inside it —
   **non-recursive by default**; `--recursive` also walks subdirectories
   (directory symlinks are never followed, so cycles are impossible). Files
   are processed in sorted path order. Output goes beside each input
   (`<stem>.tex`) or into `--out` when given (`--out` is created if
   missing); an existing `.tex` is overwritten, including collisions between
   e.g. `foo.txt` and `foo.crd` (last file in sorted order wins). Batch mode
   is inherently non-interactive: the audit report is printed per file, and
   the non-interactive defaults apply. Per-file failures (unreadable,
   non-UTF-8, empty, no song sections, write error) are reported to stderr
   and skipped, never fatal; a summary line reports `Converted N, skipped
   M.` An empty directory converts nothing and succeeds. The exit status is
   0 when every file converted and 1 when any file was skipped.
4. **Error handling.** Single-file mode turns empty input and input without
   song sections into clear hard errors; all user-facing failure paths
   (read, TUI, render, write, batch directory listing / output-directory
   creation) carry anyhow context naming the file and stage. `main` returns
   `anyhow::Result`, so an error prints `Error: …` and exits 1; success
   exits 0.

## Context

Milestone 6 (ADR0009) is the polish milestone; the advisory board promoted
batch-directory mode from stretch goal into the milestone. The batch
requirement forced an explicit definition of "what happens without review",
which exposed that `--yes` and the TUI disagreed: ADR0012 defines ambiguous
lines as "keep as lyric" (the TUI's auto-accept does exactly that), but the
render-time behavior dropped them from `--yes` output without any trace.
ADR0004 already required an audit trail for `--yes` ("a printed summary of
what was auto-discarded") to keep trusting the heuristics honest.

## Options considered

- Ambiguous lines without review:
  - **Keep as lyric, matching the TUI's default verdict (adopted)** — one
    consistent rule everywhere; the audit names every kept line so a wrong
    guess is visible in the output message, not only in the `.tex`
  - **Drop them (previous behavior)** — quieter output, but silently loses
    lyric text and contradicts ADR0012's triage default
- Batch discovery:
  - **Top-level only, opt-in `--recursive` (adopted)** — predictable, mirrors
    familiar tools; a full songbook tree is still reachable when wanted
  - **Recursive by default** — surprising when pointed at a large tree
  - **Glob/extension flags** — more CLI surface than the need justifies
- Batch per-file failures:
  - **Skip, continue, summarize, exit 1 if any skipped (adopted)** — one bad
    paste must not block a directory; scripts still see partial failure
  - **Abort at first error** — defeats the purpose of batch conversion
  - **Exit 0 even with skips** — hides partial failure from scripts
- Output naming:
  - **Beside input / `--out` directory, `<stem>.tex`, overwrite existing
    (adopted)** — matches the single-file default; overwrite is documented,
    and the `foo.txt` vs `foo.crd` collision resolves deterministically

## Consequences

- `--yes` output changes for inputs with ambiguous lines: they are now
  rendered as lyrics instead of being silently dropped. The audit lists each
  one so the change is visible, not silent.
- The TUI itself is unchanged: it still returns its reviewed song; the
  non-interactive defaults are applied by the caller only when no review
  ran.
- Batch reuses `parse`/`render` unchanged; no parser or renderer interface
  changed and no new dependencies were added.
- `tui::run`'s internal TTY check and the caller's `triage_ran` check use the
  same predicate (`stdin.is_terminal()`); they are evaluated at effectively
  the same moment, so the audit prints exactly when the TUI fell back.
- Per-file audit output in batch mode is a **single summary line** appended
  to each `Wrote …` line (e.g. `Wrote x.tex — 1 ambiguous kept as lyric`);
  the full multi-line report stays in single-file mode. Trimming batch
  output was an advisory-board condition for adoption.

## Relevant principles

None yet.

## Relevant radar blips

None yet.

## Advice

- Development agent (milestone 6), 2026-11-01: implemented the audit, the
  non-interactive defaults, batch mode, and the error-handling pass; submits
  this ADR as Proposed for the advisory board to confirm or amend. Open
  points: whether batch mode with skips should exit 1 (chosen) or 0, and
  whether the per-file audit should be trimmed in batch mode.
- Project owner (advisory board), 2026-11-01: confirmed all three open
  points — exit 1 when any batch file was skipped; ambiguous lines kept as
  lyrics in non-interactive runs (with the audit naming them); batch mode
  prints one audit line per file, full report only in single-file mode.
  Adopted with those conditions.
