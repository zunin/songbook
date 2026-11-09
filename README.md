# Songbook

Convert chord-over-lyrics text (`.txt`/`.crd`) into printable songs and
multi-song books. Rust core semantics power the CLI/TUI and a static browser
app; generated LaTeX uses the `songs` package and is compiled as-is to PDF.
Song content stays local: no uploads or session persistence.

- **[Use the live website](https://zunin.github.io/songbook/)** (desktop browsers).
  The site updates after each successful tagged-release deployment.
- **[Download releases](https://github.com/zunin/songbook/releases)**: servable
  distribution zip and corresponding source bundle.
- [Architecture](docs/architecture.md), [principles](docs/principles.md), and
  [UX context](docs/ux-context.md).

## Literal tablature (v0.3.0)

Labelled six-string ASCII guitar tabs and associated literal diagram annotations
are preserved through per-line Keep / Discard / Edit / Undo, preview and export.
Conservative recognition associates parenthetical chord/fingering cues and an
optional chord heading in either order; **Tab annotation** (`t`) can attach other
adjacent comments nearest-first. Complete supported blocks can be split at a raw
column with a numeric-entry → preview → Apply / Cancel flow (**F2** in the TUI,
**Split tab block** in web triage); Apply is one atomic undoable change. Ordinary
line editing still uses **Enter = Save**.

Printing is limited to six-string guitar, printable ASCII and **56 columns per
annotation or string row at 10pt cmtt**, without wrapping or shrinking. Original
non-six runs are rejected; partial six-string blocks retain their rows with a
warning. Tabs and annotations stay literal when chords transpose: there is no
fret transposition, tuning/musical validation or guaranteed musically safe split.
See [literal-tab scope and limits](docs/adr/0021-literal-ascii-tablature.md) and
[previewed splitting](docs/adr/0022-previewed-six-string-tab-splitting.md).

## Local development

Open the repository in its [devcontainer](.devcontainer/devcontainer.json)
(for example, VS Code's **Reopen in Container**). It supplies Rust, the wasm
target, pinned wasm-bindgen CLI, and native Tectonic.

```sh
cargo run -- path/to/song.txt
cargo build --release
bash web/build.sh
```

In the same devcontainer, build the pinned browser PDF assets (Node and
Poppler are supplied there), then serve the complete site:

```sh
bash web/build-pdf-assets.sh
python3 -m http.server -d web/dist 8080
```

Open <http://localhost:8080>. Run the web build first: it recreates
`web/dist/`; the PDF build adds assets and runs the PDF smoke test by default.
Upstream assets may be downloaded during the build, never song content.
`web/dist/` is generated, not source. `web/build.sh` requires committed or
stashed tracked changes because it packages the source from HEAD. See
[AGENTS.md](AGENTS.md) for gates.

## Roadmap: native Git issues

[git-native-issue](https://github.com/remenoscodes/git-native-issue) is the
**authoritative roadmap**, stored under `refs/issues/`, not GitHub Issues or
a Markdown status table. Run `git issue ls --all` for the current roadmap;
issue descriptions distinguish feature requests from approved implementation
scope.
[Migration and historical context](docs/roadmap.md).

```sh
brew install remenoscodes/git-native-issue/git-native-issue
git issue init origin
git issue merge origin
git issue ls --all
git issue show 684bf28
```

An ordinary clone needs `git issue init origin` to add the issue fetch
refspec, then fetch/merge (`git issue merge origin` does both). Initialize
once per clone; merge again before updating or publishing issues.

```sh
git issue create --help
git issue comment --help
git issue state --help
```

After creating or updating roadmap issues, publish them explicitly:

```sh
git issue merge origin
git issue fsck
git push origin 'refs/issues/*:refs/issues/*'
```

Commit and push README/AGENTS/roadmap documentation separately when changed.
Ordinary branch pushes **do not publish issue refs**. Never force-push issue
refs; merge divergent histories instead. Custom refs are not included in
source archives, so use a Git clone to obtain the live roadmap. The tool's
GitHub import/export bridge is optional and manual: there is no automatic
sync, and the native roadmap remains authoritative.

## Licensing

See [LICENSE](LICENSE), [third-party notices](THIRD_PARTY_NOTICES.md), and
`licenses/`. Local copyrighted tabs must not be staged or copied into tests;
use synthetic examples.
