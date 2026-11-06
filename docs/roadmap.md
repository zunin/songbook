# Roadmap

Feature priorities are decided by the project owner; the architecture
facilitator maps them to architecture needs and the advice process.
Last reconciled: 2026-11-06.

## Remaining feature work

| Direction | Scope still to agree | Process need |
|---|---|---|
| **PDF options** | Paper size selection, font choices, and per-song typesetting/layout options using the `songs` package. Specific controls and defaults have not yet been agreed. | Product discussion and UX advice before implementation. Basic controls are mostly below threshold; new font dependencies, pack contents, or shared configuration formats require board advice. Pack changes must update and verify the pins deliberately. |

PDF output itself is shipped; adopting the engine did not deliver these
configurable options. This is the remaining item from the original roadmap.

## Completed feature work

| Direction | Delivered |
|---|---|
| **Songbooks (multi-song)** | Core book composition and CLI manifests (ADR0018); browser books with real title/chord indexes (ADR0019), mixed file/pasted sources, paste-only book starts, and resume at unfinished songs. |
| **Paste input** | Browser single-song paste and pasted sources in books (ADR0020); no Clipboard API, uploads, or persistence. The TUI remains file-based. |
| **Transposition** | Core column-preserving rewrite; CLI single/batch/book support; web and TUI preview controls; key-aware spelling with ladder fallback; guarded Key metadata updates; 6/9 chord support. |
| **UX review** | Findings implemented across v0.2.1 and v0.2.2 or explicitly closed by board decision. See `docs/ux-review-2026-11-03.md` and `docs/ux-context.md`. |
| **Website distribution** | GitHub Pages deployment on version tags; GitHub Releases carry a servable dist zip and corresponding source bundle. |

## Release verification

**v0.2.2 verified 2026-11-06:**

- [Actions run 41591958287](https://github.com/zunin/songbook/actions/runs/41591958287)
  completed successfully.
- [Release v0.2.2](https://github.com/zunin/songbook/releases/tag/v0.2.2)
  carries `songbook-dist-0.2.2.zip` and `songbook-source-0.2.2.tar.gz`.
- [The live site](https://zunin.github.io/songbook/) serves the app, PDF asset
  manifest, and versioned source bundle (HTTP 200).
- The project owner approved the local v0.2.2 UX changes before release.
  HTTP verification confirms deployment, not a new browser interaction test.

## Accepted limitations, not open roadmap items

- TUI line editing remains append-only; mid-line editing is available in the
  browser. The board closed the full TUI cursor model.
- The web triage focus model is documented and closed; no further redesign
  is scheduled.
- Desktop browser scope, no autosave/persistence, and no URL fetching remain
  deliberate boundaries, not implicitly planned features.

## Working agreement

Threshold items open with an option paper and board advice (the project
owner holds a veto). Below-threshold implementation is delegated, verified,
and recorded when it deserves a record. User-facing changes receive UX
advisor review before implementation.
