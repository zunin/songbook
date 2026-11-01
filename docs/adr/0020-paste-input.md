# ADR0020 — Paste input (browser app)

## Status

Adopted (2026-11-01)

## Decision

The browser app accepts pasted tab text through a labelled textarea on the
load screen ("Use pasted text"), which starts the same single-song flow as
dropping one file. In book mode, pasted songs can join a book: the book's
sources are entries that are either a file or pasted text, triaged in order
like any other song. The terminal UI stays file-based. URL fetching remains
excluded (ADR0005). This decision supersedes ADR0005's interactive-paste
clause only.

Privacy (P3) is unchanged: pasted text lives in memory, feeds the same parser
path as file contents, and is never sent anywhere or persisted. The app does
not use the Clipboard API (`navigator.clipboard.readText()`); the user pastes
with the ordinary browser gesture into a field the app renders, so nothing is
read implicitly.

## Context

ADR0005 chose files over paste-into-the-TUI and URL fetching when the product
was terminal-only: paste "complicates the TUI and adds no parsing capability",
and the manual save step was acceptable. The browser app (ADR0016) did not
exist then; a textarea there is a small surface, and the browser is where
tabs are copied from. ADR0016's limitations list names "No paste input
(ADR0005 boundary)" and is updated by this decision.

## Consequences

- The load screen gains a labelled textarea and button; the book flow gains
  "add a pasted song" alongside files.
- Book entries are modelled as sources (file or text); the CLI, batch mode and
  TUI are unaffected.
- Download names for pasted songs derive from the song title (no filename
  exists), falling back to "song".
- No new dependencies, no clipboard permissions, no storage; nothing changes
  for the `.tex`/PDF pipeline or the privacy boundary.
- A very large paste costs the same as a very large file; no artificial cap
  is introduced.

## Advice

- **Advisory board** (project owner), 2026-11-01, forum session five: chose
  option A (load-screen textarea, no clipboard API) and enlarged the scope to
  include pasted songs inside books. No veto.

## Limitations (accepted)

- Pasted songs in a book are collected as sources; there is no rich source
  editor (reordering/removing stays as it is for files: start over).
- The TUI still requires a file.
- URL fetching remains out of scope (ADR0005).
