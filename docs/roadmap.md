# Roadmap migration and history

The project owner adopted **git-native-issue as the authoritative roadmap**
on 2026-11-06. Live priorities, scope, progress, and state live in Git's
`refs/issues/`, not this document or GitHub Issues.

**`684bf28` — Define and implement PDF options** carries the
remaining direction from the original roadmap. PDF output already shipped;
configurable paper sizes, fonts, and per-song layout are candidates whose
controls, defaults, and surfaces require owner choice and UX advice. Read the
issue for the agreed scope and acceptance evidence; this page does not mirror
its live status.

```sh
git issue init origin
git issue merge origin
git issue ls --all
git issue show 684bf28
```

See [README](../README.md#roadmap-native-git-issues) for installation, clone
setup, explicit issue-ref publication, and optional manual bridges;
[AGENTS.md](../AGENTS.md) defines the advice and verification workflow.

## Completed-feature history (migration snapshot)

- Multi-song books: core composition and CLI manifests (ADR0018); browser
  title/chord indexes, mixed file/pasted sources, paste-only starts, and resume
  at unfinished songs (ADR0019).
- Browser paste input for single songs and books (ADR0020), without Clipboard
  API, uploads, or persistence. TUI remains file-based.
- Transposition: column-preserving core rewrite; CLI single/batch/book and
  web/TUI previews; key-aware spelling with ladder fallback, guarded Key
  metadata updates, and 6/9 chord support.
- UX findings delivered in v0.2.1/v0.2.2 or explicitly closed by board decision:
  see [UX review](ux-review-2026-11-03.md) and [UX context](ux-context.md).
- Website distribution: version-tag GitHub Pages deployment; releases carry
  a servable distribution zip and corresponding source bundle.

## Release evidence retained

**v0.2.2 verified 2026-11-06:**
[Actions run 41591958287](https://github.com/zunin/songbook/actions/runs/41591958287)
succeeded. [Release v0.2.2](https://github.com/zunin/songbook/releases/tag/v0.2.2)
contains `songbook-dist-0.2.2.zip` and `songbook-source-0.2.2.tar.gz`.
The [live site](https://zunin.github.io/songbook/), PDF asset manifest, and
versioned source bundle returned HTTP 200. The owner approved the local UX
changes before release; HTTP verification was deployment evidence, not a new
browser interaction test.

## Accepted limitations, not implicitly planned work

TUI editing is append-only (mid-line edits belong to the browser); the board
closed the full TUI cursor model. The web triage focus model is documented
and closed. Desktop-only browser scope, no autosave/persistence, and no URL
fetching are deliberate boundaries. See [UX context](ux-context.md) for the
full limitations and interaction conventions. Do not treat these as open
issues unless the owner revisits them or a change worsens them.
