# ADR0022 — Previewed six-string tab splitting

## Status

Adopted (2026-11-07), amended for associated diagram annotations.
Implementation accepted by the owner on 2026-11-09 after automated integration
checks and visual review of annotated tabs and splitting. Release remains pending.
Accessibility evidence is automated checks and owner review, not a recorded
hands-on screen-reader test.

## Decision

Add an explicit **core plan → preview → apply** operation for complete six-string
blocks descended from supported original six-row groups. This is a block operation,
not physical multiline editing or a change to ordinary line-based review.

The user chooses a shared raw column count **N**. Columns count indentation,
string label and first pipe, not musical positions. For each row, let **p_i** be
its exact prefix length through the first pipe, **P = max(p_i)**, and **L** the
maximum current row length. Require **P < N <= 56** and **N < L**.

Core computes both ordered six-row children from the current literal rows:

- **Left:** the first N raw columns, or the whole row when shorter.
- **Right:** the repeated exact prefix, then **P - p_i** ordinary spaces, then
  the original suffix after N (empty for a row no longer than N).

Preserve every suffix character and trailing space; never trim tails, padding,
notes or other musical text. Prefix repetition and alignment padding are the only
additions. The preview shows both complete children before explicit Apply.

A right child may remain wider than 56 columns: warn, allow Apply, and allow the
same operation again on a complete child. Export's width rejection remains intact.
Short rows may produce label-only right rows; disclose this in the preview and
retain them by stable block identity, rather than reparsing their text as a new
source group. Both children share stable original provenance but have distinct
block-instance identities, preventing unrelated groups from merging.

Reject original non-six runs and partial, reclassified or fragmented groups.
Every plan is bound to the current data and selection; any change makes it stale,
and core rejects stale Apply without mutation. Apply replaces the whole block
atomically with left then right. It is **one undo step**, restoring all original
row states, flags, position and provenance. Cancel makes no changes.

The TUI uses **F2** for a labelled numeric-entry and preview flow: Enter previews
from numeric entry, then Enter applies from the preview. The web exposes a
contextual **Split tab block** button with the same core plan, numeric entry and
separate **Preview split**, **Apply split** and **Cancel split** controls.
**Escape** cancels; ordinary line editing keeps **Enter = Save**. Controls and
warnings must be keyboard-operable, labelled and announced where supported.
Core alone computes and replaces rows; neither UI invents musical text.

### Annotation-aware amendment

Associated literal annotation rows are included in the same core plan and
atomic replacement as their six strings. Selecting an eligible annotation or
string exposes the same operation. The preview and output order is **left
annotations, left six strings, right annotations, right six strings**.

Annotation rows are sliced at the same raw column. Their right fragments
contain only the original suffix plus the disclosed alignment spaces needed
to match the repeated string prefixes; whole annotation text is never repeated.
Empty fragments are described outside the literal content. Prefix width P is
still derived from strings, while overflow checks include annotation lengths.
The preview warns that the cut can divide annotation text as well as notation.
Roles, source provenance, flags and every row decision are preserved by Undo.

## Context

Native Git issue `84e5e08` tracks literal tablature. After human acceptance that
the initial tablature looked great, the owner requested splitting wide six-string
blocks: Enter in line editing cannot insert properly ordered left/right blocks.
That initial feedback did not constitute acceptance of the new operation; final
owner approval followed on 2026-11-09 (see Advice).

This amends ADR0021's oversized-tab rejection remediation: explicit manual splitting
is useful, while automatic wrapping, clipping, shrinking and whitespace
normalization remain forbidden. ADR0021's literal rendering, export bounds,
original-group provenance and unchanged-tab transposition rules otherwise stand.

The operation crosses triage, provenance, undo, wasm and both interactive surfaces.
P2 requires shared core semantics; a physical Split line action would not preserve
six-string ordering or provide an atomic block preview and undo.

## Consequences

- Preview and Apply share one core plan, with validation at Apply; UI-specific
  slicing or inferred musical boundaries cannot diverge across surfaces.
- Stable origin and child-instance identity preserve supported descendants,
  label-only rows and repeated splits without laundering unsupported source runs.
- Raw-column splitting can cut a multi-digit fret or technique token. Show an
  explicit warning before Apply: **no musically safe cut is guaranteed**. The user
  must inspect and accept the literal result; there is no interpretation heuristic.
- No new dependencies, catalogue, playback or transposition interpretation.
  Client-only privacy, pinned assets and core's render-once invariant remain intact.
- Synthetic core tests for boundaries, exact whitespace, repeated splits,
  rejection, stale plans and full atomic undo passed, alongside devcontainer
  workspace/wasm-target gates, browser build and native/browser PDF checks.
  Automated interaction/accessibility checks and owner visual review support
  implementation acceptance on 2026-11-09; no hands-on screen-reader test is
  claimed. Release remains pending.

## Advice

- **Architecture and UX advisors**, 2026-11-07: recommended a block operation
  separate from physical Split line, with explicit preview and atomic application.
- **Project owner**, 2026-11-07: explicitly adopted raw-column plan/preview/apply
  on both surfaces, atomic undo, disclosed label-only rows, and repeat splitting
  when the right child remains oversized.

- **Annotation amendment**, owner and architecture/UX advisors, 2026-11-07:
  approved annotations travelling with both split children, same raw-column
  cut and disclosed right alignment spaces; never duplicate whole comments.
- **Integration verification**, 2026-11-09: devcontainer workspace/wasm-target
  gates and browser build passed, including real optimized wasm planning,
  cancellation, apply and undo checks. Annotated standalone, split standalone
  and split-book exports are covered by seven browser-worker PDF cases matching
  native text/layout; all 18 tested native/browser raster pages are byte-identical.
  All source fixtures are synthetic. Automated interaction/accessibility checks
  passed; no hands-on screen-reader test or public deployment is claimed.
- **Project owner final approval**, 2026-11-09: after visual review of annotated
  tab inclusion and splitting, “It looks great! No notes”. Implementation is
  accepted; release and public-site deployment remain pending.

## Limitations (accepted)

- This is manual literal restructuring, not a general multiline or musical editor.
- An accepted split need not make export eligible; remaining overflow and other
  ADR0021 restrictions still apply. Existing TUI append-only editing is unchanged.
