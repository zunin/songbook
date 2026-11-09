# ADR0021 — Preserve literal ASCII guitar tablature

## Status

Adopted (2026-11-06), amended for diagram annotations (2026-11-07).
Implementation accepted by the owner on 2026-11-09 after automated integration
checks and visual review of annotated tabs and splitting. Release remains pending.
Accessibility evidence is automated checks and owner review, not a recorded
hands-on screen-reader test.

## Decision

Retain and render **literal, labelled six-string guitar tablature**. Alternative
string labels are accepted without tuning or musical validation. Preserve raw
spaces (including indentation and trailing spaces) and printable characters;
do not interpret frets, notes, rhythm, techniques, or tuning.

Review stays line-based: **Keep / Discard / Edit / Undo**, not an atomic block
editor. Pending tab lines are retained; bulk acceptance and non-interactive
conversion (`--yes`, non-TTY fallback, batch) retain them too. This supersedes
ADR0013's fretboard-dropping default and corresponding dropped-tab audit rule;
other defaults are unchanged. Retention does not guarantee export eligibility.

Core records original block provenance in shared `Song` / `TabGroup` data before
triage. Blank/non-tab source lines separate runs; adjacent complete six-row
blocks may be split every six rows. Original non-six runs remain unsupported,
not reinterpreted as valid blocks. Removing separators must not merge distinct
source groups. Discarding rows from a valid six-row group yields a **partial tab
warning**, preserves the retained rows, and never invents missing strings.

Export accepts only printable ASCII (bytes 32–126), bounded to **56 columns per
row at readable 10pt cmtt**. Reject oversized or unsupported notation with an
actionable error naming the song, block/source location and remedy: shorten
indentation, split into aligned six-string blocks, edit, discard or reclassify.
Never silently wrap, shrink, truncate, normalize whitespace or drop notation.

Chord/key transposition leaves tabs unchanged. When a non-zero transposition
and retained tablature coexist, show and announce the shared conditional warning:
“Chord transposition is active; ASCII tabs are unchanged and may no longer match.”
Partial-block and export warnings likewise come from core, not UI heuristics.

Reuse existing standalone/body/book renderer entry points. Tab-bearing standalone
songs and books activate the `songs` package's one-column layout. Render literal
rows through fixed TeX structure and numeric glyph tokens, never raw source TeX.
Measure spaces using the selected font's cell width; use cmtt's straight quote
and backtick slots (13 and 18), rather than its curly OT1 slots (39 and 96).
Different tab choruses must not deduplicate by ignoring their notation.

### Diagram annotation amendment

Aligned chord headings and parenthetical chord/fingering cues can belong to
an ASCII diagram rather than the song's chord-over-lyric content. Core uses a
separate literal `TabAnnotation` kind and role-specific provenance: annotations
never count as string rows, become lyrics, transpose, or enter the chord index.

Automatic association is conservative: an eligible parenthetical chord/fingering
cue and an optional chord-heading row must be contiguous with an original
supported six-string run. Both cue/heading orders are recognized; a heading may
contain the explicit alternative separator `or`. Blank, section, metadata and
unrelated-text barriers are not crossed. Lone chord headings and ordinary
parenthetical lyrics are not sufficient evidence.

The user can explicitly expand a diagram with the **Tab annotation** action
(`t` in triage), attaching adjacent comments nearest-first through core's
eligibility rules. Keep, Discard, Edit and Undo stay per row. Annotations retain
literal whitespace and their own source identity; removing one never causes a
false missing-string warning. Orphan annotations are retained with a warning.
Each retained annotation row is independently checked against the same ASCII
and 56-column print policy. Annotation-aware splitting is recorded in ADR0022.

## Context

Native Git issue `84e5e08` concerned tabs classified as fretboard material but
originally omitted from printable output. File and pasted sources (ADR0020)
need the same fidelity through review, preview, standalone export and books.
Literal retention addresses this without introducing a musical notation engine.

The change crosses parser/rebuilder provenance, shared warnings, preview, wasm,
TUI/web views and print layout, so it requires owner, architecture and UX advice.
P1 requires every PDF path to compile core's exact `.tex`; P2 keeps all semantic
choices in core. JavaScript receives shared warnings and the preview tablature
style through wasm: it implements neither musical nor LaTeX semantics.

## Consequences

- Shared provenance prevents discarded boundaries from accidentally joining
  unrelated blocks; undo and editing must preserve the original-group distinction.
- Preview uses a literal monospaced tablature style. Thin TUI/web views expose
  the same retention, partial-block and transpose warnings with labelled,
  keyboard-operable controls and visible/spoken feedback where supported.
- Tab-bearing output trades two-column density for predictable readable width;
  ordinary non-tab output retains its existing layout.
- Safe fixed numeric glyph rendering preserves punctuation and alignment without
  permitting TeX commands, comments or delimiter injection from source text.
- No new dependencies, fonts or TeX packages were added; existing pins, pack
  sources, notices, client-only privacy and render-once invariants remain intact.
- Synthetic production regression tests, devcontainer workspace/wasm-target gates,
  browser build and native/browser PDF checks passed (see Advice). The owner
  accepted the final visual result on 2026-11-09. Automated interaction and
  accessibility checks plus owner review are recorded; no hands-on screen-reader
  test is claimed. Continue using synthetic fixtures only, never excluded
  copyrighted example content.

## Advice

- **Architecture and UX advisors**, independently, 2026-11-06: recommend tabs
  ahead of additional PDF options and a fingering index; favour literal retention
  with bounded printing, shared core semantics and explicit limitations.
- **Project owner**, 2026-11-06: approved literals, labelled six-string scope,
  retention defaults, line-based review, partial/transpose warnings and blocking
  overflow; alternative labels do not imply tuning validation or bass support.
- **Independent native-print feasibility review**, 2026-11-06: measured a cmtt
  cell at 5.24995pt; 56 columns occupy 293.99744pt. Adding 6pt note padding and
  6pt chorus indentation gives 305.99744pt, within a conservative 345pt width
  (the actual 12pt article width is 390pt). A six-row note measured 75.16667pt;
  scratch page tests passed. These were feasibility results, not production,
  native/browser regression or owner acceptance; subsequent verification and
  acceptance are recorded below.

- **Annotation amendment**, owner and architecture/UX advisors, 2026-11-07:
  approved conservative diagram-comment recognition plus explicit manual
  expansion, separate annotation roles, per-row review and aligned split
  fragments. This addresses the owner's screenshot without interpreting
  musical fingerings or broadly absorbing nearby lyrics.
- **Integration verification**, 2026-11-09: devcontainer workspace/wasm-target
  gates and browser build passed. Seven browser-worker PDF cases match native
  text/layout, and all 18 tested native/browser raster pages are byte-identical.
  Source fixtures are synthetic. Automated interaction/accessibility checks
  passed; they do not constitute a hands-on screen-reader test or deployment.
- **Project owner final approval**, 2026-11-09: after visual review of annotated
  tab inclusion and splitting, “It looks great! No notes”. Implementation is
  accepted; release and public-site deployment remain pending.

## Limitations (accepted)

- ASCII only: tabs, control characters and Unicode block export. Guidance must
  direct users to edit whitespace to ordinary spaces or remove/reclassify material.
- No bass support is promised; original non-six runs are unsupported even if
  review removes enough rows to leave six. Partial warnings are not musical checks.
- No interpreted notes, playback, tuning validation or fret transposition; users
  must reconcile unchanged tabs with transposed chords themselves.
- TUI editing remains append-only (the accepted UX boundary), not a block editor.
- The general `songs`-package stanza-layout limitation for arbitrarily tall songs
  remains; bounded tab width is not a promise that every song fits one page.
