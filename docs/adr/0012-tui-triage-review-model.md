# ADR0012 — TUI triage review model and metadata provenance

## Status

Adopted (the `a`-key deferral is resolved; the flattened-Song review model and
flagged-only queue are superseded in part by ADR0014)

## Decision

1. **Separate review model.** The triage screen does not edit `Song` sections
   in place. `TriageState` flattens the parsed `Song` into an ordered list of
   review items (section boundaries plus content lines), each with its original
   `LineKind`, a current `Verdict`, and optional edited text. Decisions are
   applied by rebuilding `Song` sections from that list when triage finishes.
2. **Review queue.** Lines whose classifier result is `Ambiguous` or
   `Fretboard` are queued. Ambiguous lines default to "keep as lyric"; every
   other line keeps its classifier verdict. Noise, blanks, and structured
   lines are not queued, matching ADR0004's "only what the machine is unsure
   about".
3. **Metadata provenance without a schema change.** `Song` is unchanged.
   `classify::metadata_provenance(raw)` rescans the input for explicit
   `Song:`/`Title:` and `Artist:`/`Band:` labels; `TriageState` marks the
   title/artist as inferred when they came from bare preamble lines. Title,
   artist, key, capo, and tuning are all editable; empty values clear the
   field.
4. **Pure state machine.** Key handling is expressed as
   `TriageState::handle(Key)`, independent of crossterm. Rendering and
   crossterm event mapping live in `src/tui/triage.rs` and `src/tui/mod.rs`.
   Undo stores snapshots of metadata and review items.
5. **Graceful non-interactive fallback.** If stdin is not a TTY, the TUI prints
   a notice to stderr and returns the heuristic song unchanged, exactly like
   `--yes`. `--yes` still bypasses the TUI entirely.

6. **Open point, deferred.** Whether `a` (auto-accept remaining) finishes
   immediately or stays on screen with undo available is deferred to
   milestone 5, when the preview screen makes the cost of a mistake visible.
   Until then the agent's implemented behavior (finish immediately) stands.

## Context

ADR0004 requires line-by-line triage over the shared `Song` model, and ADR0010
requires inferred metadata to be correctable in that flow. Direct mutation of
`Song.sections` is awkward: removing a line shifts indices, promoting a line to
a section header restructures sections, and undo would need to reconstruct the
previous structure. Preamble noise and blank lines are already excluded from
`Song`, so the shared model alone cannot provide full raw context either.

## Options considered

- **Flattened review list + rebuild (adopted)** — stable review indices,
  straightforward undo, section headers and edits handled by one rebuild
  function; `Song` remains the single export model.
- **Mutate `Song` in place with stable IDs** — avoids a rebuild pass, but every
  verdict needs bespoke structural edits and undo bookkeeping, with more edge
  cases around section splitting.
- **Add triage fields to `Song`/`LineKind`** — would leak review concerns into
  the parser/renderer model, which ADR0009 intentionally stabilized first.
- **Detect metadata provenance by adding a parser field** — rejected for the
  same reason; a read-only scan of raw labels gives the TUI what it needs
  without changing the shared schema.

## Consequences

- The final song is rebuilt once at the end of triage; metadata is applied to
  the same rebuilt `Song` before rendering.
- Reclassifying a line to "section header" starts a new section at that point;
  reclassifying to lyric/chord-only/noise/discard is a per-item decision.
- Context in the triage pane comes from the flattened review list, not from
  raw lines already discarded as noise or merged by the parser.
- Metadata provenance is a heuristic scan; a label-like string in lyric text
  could, in principle, mark a field as labelled. The TUI still shows the value
  for editing, so the consequence is only a missing "(inferred)" hint.
- The state machine is unit-testable without a terminal; rendering uses
  ratatui `TestBackend`.

## Relevant principles

None yet.

## Relevant radar blips

ratatui `TestBackend`: adopt (already part of ratatui). No new dependencies.

## Advice

- Development agent (milestone 4), 2026-11-01: implemented the review model,
  metadata panel, key state machine, non-TTY fallback, and tests; submits this
  ADR as Proposed for the advisory board to confirm or amend.
- Project owner (advisory board), 2026-11-01: confirmed the review model as
  adopted, including fretboard lines in the queue by default; `q` types a
  character during metadata text entry (Esc/Ctrl+C quit there); `s` stays a
  distinct finish-now key (becomes "skip to preview" at milestone 5);
  reclassified section headers keep the user's typed text rather than
  snapping to a fixed list; `a`'s finish-vs-stay behavior deferred to
  milestone 5.
- Assistant (facilitating milestone 5), 2026-11-01: after the board deferred
  the `a`-key choice to milestone 5, the assistant decided that `a` accepts
  remaining flagged lines and jumps to preview. Esc/b returns to triage with
  full undo available, so a bad auto-accept is recoverable before export.
  Implemented and tested (`auto_accept_keeps_remaining_and_previews`,
  `preview_back_returns_to_triage_with_state_intact`).

## Errata

- Assistant (facilitator), 2026-11-01: the second decision item was numbered
  `5.` in the original text, duplicating item 5; renumbered to `6.`. The Advice
  record originally said the assistant made the `a`-key choice "as the board
  instructed." The board had actually deferred the choice to milestone 5; the
  assistant then made the specific choice recorded above. The implementation
  and decision are unchanged.
