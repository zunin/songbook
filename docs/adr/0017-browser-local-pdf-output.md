# ADR0017 — Browser-local PDF output via Tectonic-in-WASM

## Status

Adopted (2026-11-01)

## Decision

The browser app (ADR0016) gains **PDF output compiled entirely in the
browser**: Tectonic compiled to WebAssembly, obtained from the pinned
`glyphtex-engine` npm package, compiles the same `.tex` the app already
produces. There is still no server and no upload of song text; TeX text
reaches the engine worker via `postMessage` only, and all engine assets
(wasm, TeX bundle, songbook TeX pack, fonts) are fetched same-origin from
`pdf-assets/` — never beyond the app's own origin. The engine is
**lazy-loaded**: nothing PDF-related is fetched until the user first asks
for a PDF. The PDF control sits beside the existing `.tex` download.

Terms accepted by the advisory board (2026-11-01):

- PDF button beside the `.tex` download; the engine loads only on first click.
- **~19 MB compressed lazy payload accepted** and recorded as the budget:
  `tectonic.wasm` ≈ 3.5 MB + `bundle.tar.gz` ≈ 13.4 MB + pack ≈ 1–2 MB. Never
  preloaded.
- Vanilla-JS integration; no new frontend dependencies.
- Assets are built **only** by `web/build-pdf-assets.sh` (hash-pinned,
  offline-replayable). Pinned hashes live in git (`web/pdf/pinned-hashes.txt`);
  build output (`web/dist/`) is ignored.
- The TeX pack (`songs.sty`, `size12.clo`, BaKoMa CM fonts) is generated from
  pinned upstream sources by the repo's docstrip emulator
  (`web/pdf/tools/docstrip.py`), which must keep failing hard on hash
  mismatch. The emulator is accepted, permanent scope — not scaffolding.
- Licensing: GlyphTeX is treated conservatively as **GPL-3.0** (npm metadata
  says MIT; the upstream repo's LICENSE at the pinned commit is GPL-3.0 and is
  carried). Complete corresponding sources ship with the built assets and are
  recorded in the build manifest. The project is GPL-3.0-or-later (LICENSE),
  so this adds no new obligations, but the source-carrying rule binds: no PDF
  assets ship without their manifest and sources.

## Context

ADR0007 chose Tectonic as the LaTeX verification toolchain; ADR0016 defined
the client-only browser MVP with `.tex` download only and named PDF a "later
stretch goal". The spike implementing that goal (`web/pdf-engine.js`,
`web/pdf-worker.js`, `web/build-pdf-assets.sh`, `web/pdf/`) was validated by a
Node smoke test that compiles a pinned fixture and requires the extracted PDF
text to match the native Tectonic reference byte-for-byte.

Alternatives considered and rejected:

- **Server-side compile**: violates ADR0016's no-server/no-upload boundary.
- **Pure-JS PDF writer** (no TeX): a second rendering of truth that would
  drift from the `songs`-package LaTeX output this project treats as its
  format (ADR0002). The small payload is not worth the fidelity risk.
- **Defer**: the validated spike made adoption cost bounded; deferral risks
  bit-rot.

## Consequences

- **WYSIWYG by construction**: the browser compiles the same `.tex` that
  ADR0007's native verification compiles — one rendering of truth.
- New permanent moving parts: the pinned asset build (fetch → sha256 pins →
  docstrip → manifest) and the worker protocol (`ready` / `compile` /
  `result` / `poisoned`). A poisoned compile kills and respawns the worker;
  the UI must surface that as a recoverable error, not a dead end.
- **Fitness function**: the Node smoke test (`web/pdf/test/smoke-node.mjs`)
  comparing extracted PDF text against the native reference is the adoption
  gate and must stay byte-exact; it runs as part of the asset build.
- `THIRD_PARTY_NOTICES.md` moves the GlyphTeX / songs / BaKoMa rows from "NOT
  BUNDLED" to bundled, referencing the manifest and carried sources, when the
  assets first ship.
- Users who never click PDF are unaffected: the payload loads only on demand.

## Advice

- **Advisory board** (project owner), 2026-11-01, architecture advisory
  forum: advised **adopt**; chose the beside-the-`.tex`-download placement;
  accepted the ~19 MB lazy payload; raised no veto (the board holds an
  absolute veto at the architecture threshold).
- The option paper was prepared by the architecture facilitator from the
  implementer's in-flight work. Integration details below the threshold
  (button states, diagnostics UX, code organization) were left to the
  implementing agent.

## Limitations (accepted)

- ~19 MB one-time payload for PDF users; no PDF output without it.
- The first compile in a session pays engine init (fetch + wasm instantiate
  + asset load); later compiles are fast.
- BaKoMa CM type only — no font selection; output matches the verified
  reference, not user fonts.
- One compile at a time; the UI guards against concurrent compiles (the
  engine API does not queue).
- The smoke gate compares extracted text (`pdftotext`), not pixels.
