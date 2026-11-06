# Working on Songbook

## Context and roles

Read [architecture](docs/architecture.md), [principles](docs/principles.md),
[UX context](docs/ux-context.md), and relevant [ADRs](docs/adr/) first.

- `crates/songbook-core`: parsing, triage, transposition, book composition,
  and LaTeX rendering. Core owns semantics and shared settings.
- `src/`: thin CLI/TUI; `crates/songbook-wasm`: browser Session API over core.
- `web/`: vanilla JS/CSS, client-only UI and lazy PDF worker/engine.
- `web/pdf/` and `web/build-pdf-assets.sh`: pinned engine/TeX pack pipeline.
- `.devcontainer/`: Rust/wasm/native Tectonic verification environment.

The **project owner** chooses priorities, product scope/defaults and surfaces,
and holds an absolute veto on threshold decisions. The **architecture
facilitator** maintains shared context, maps work to architecture needs, and
coordinates advice/records. **Architecture and UX advisors** review trade-offs
and user-facing work. **Delegated implementers** decide below-threshold details,
implement the agreed scope, verify it, and record the outcome.

Seek owner/architecture advice **before implementation** for new dependencies,
components, public APIs, file/pack/config formats, cross-cutting patterns, or
other hard-to-reverse changes. Use an option paper and record advice in an ADR.
User-facing changes require UX review before implementation; follow the labelled,
keyboard-operable, screen-reader-aware conventions in the UX context.

## Authoritative roadmap and issue workflow

Native Git issues, not Markdown tables or GitHub Issues, are authoritative.
See [README](README.md#roadmap-native-git-issues) for installation and sync.
Use `git issue ls --all` to discover the current roadmap and read the relevant
issue rather than assuming one hard-coded task. Feature requests are not
approved implementation requirements until the owner agrees their scope.

Before work:

```sh
git issue init origin
git issue merge origin
git issue ls --all
git issue show 684bf28
```

Read the relevant issue and comments, agree scope with the owner, and obtain
advice at the threshold. Comment progress, decisions, blockers, and verification
on the issue (`git issue comment --help`). Before closing, reference the actual
implementation commit and the acceptance evidence (`git issue state --help`);
do not close work merely because code was written. Accepted limitations are
not unfinished roadmap tasks: do not reopen/re-flag them unless worsened or
the owner revisits the decision. Historical context lives in
[docs/roadmap.md](docs/roadmap.md), not a duplicate live status list.

Merge divergent issue histories with `git issue merge origin`; **never
force-push issue refs**. After creating or updating roadmap issues, publish
them when authorized by the owner:

```sh
git issue merge origin
git issue fsck
git push origin 'refs/issues/*:refs/issues/*'
```

Ordinary branch pushes do not publish issues. Commit/push changed documentation
separately, and verify the remote issue refs before claiming publication.
Do not commit or push unless authorized by the coordinating owner.

## Invariants and verification

Core renders `.tex` once; every PDF path compiles that exact artifact. Never
implement LaTeX semantics in JavaScript. No song-content uploads, storage APIs,
or persistence. Keep surfaces dependency-light. Preserve hash pins, deterministic
builds, notices, and corresponding sources; change pins only deliberately with
advice and verification.

Run full Rust gates inside the devcontainer (do not substitute host-only tests):

```sh
cargo fmt --all -- --check
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
```

For affected browser/wasm work, run `bash web/build.sh` in the devcontainer.
For affected PDF/pack/rendering work, then run `bash web/build-pdf-assets.sh`
in the devcontainer with its default smoke test enabled. Preserve golden tests and
native/browser PDF comparison; do not bypass failing pins or smoke checks.
User-facing work also needs keyboard, focus, labels, live-region and
screen-reader/accessibility verification. Report commands actually run and
any unverified gates explicitly.

Copyrighted local `examples/{harvest,iris,let-down}.txt` are excluded locally:
**never stage them, copy their content, or turn them into fixtures**. Tests
must use synthetic material. `web/dist/` and caches are generated artifacts,
not source. Inspect the diff and staging list before handing work off.
