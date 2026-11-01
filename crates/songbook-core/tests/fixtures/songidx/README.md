# songidx golden fixtures (ADR0019)

Each pair `NAME.sxd` / `NAME.sbx` pins one conversion of the
`songbook-core::songidx` port: the `.sxd` is the input (in the exact format
the songs package's LaTeX pass 1 writes), the `.sbx` is the **native**
`songidx.lua` output the port must reproduce byte for byte (ADR0008-style
golden: regenerating is a deliberate, auditable act).

## How the goldens were generated

Inside the devcontainer image (`songbook-devcontainer:latest`), which has the
pinned songs package sources in `web/dist/pdf-assets/sources/songs.zip`:

```sh
unzip -p web/pdf/.cache/songs.zip songs/songs/songidx/songidx.lua > songidx.lua
apt-get update && apt-get install -y texlive-binaries   # provides texlua
LC_ALL=C texlua songidx.lua NAME.sxd NAME.sbx          # for each fixture
```

- Interpreter: **texlua** (LuaTeX 1.15 / Lua 5.3.6, TeX Live 2022 as packaged
  in Debian 12) — the interpreter the songs package documentation itself
  prescribes (`texlua songidx.lua …`), which supplies the `unicode.utf8`
  library songidx.lua depends on.
- Locale: **`LC_ALL=C`**, so Lua's `strcoll`-based string comparisons are
  plain byte order; the port hard-codes that same byte-wise comparison (see
  the module documentation for why other locales are out of scope).

## Fixture list

- `plain` — ASCII titles across several blocks (basic sort + block churn).
- `articles` — `The`/`A` rotation, non-rotation (`An`, `Ain't`, `Theme`),
  bare `A`/`The`, double space after the article, lowercase prefix words.
- `accents` — raw UTF-8 titles (é, ß unchanged under upper, Æ/É/Ü/北 block
  characters in byte order).
- `numbers` — Lua `tonumber` grammar through `numprefix` (`1.5x`, `0x1C`,
  `10,000` → `10`), numbers-before-words, and the block-character quirk that
  relabels a block when a later entry's start character is smaller.
- `chords` — chord entries exactly as `songbook-core::book` emits them
  (`*Am`, `*D/F\#`, `*C°`, `*Cø`, `*N\.C\.`, …), duplicate merging, and
  `\idxaltentry` output.
- `dups` — duplicate titles merging into `\\\songlink` continuations;
  case-differing titles (`Song`/`song`) staying separate entries.
- `altmixed` — normal entries, `*`-prefixed alternates, and merging
  interleaved (alternates sort after equal normal entries).
- `macros` — LaTeX-escaped titles (`\&`, `\%`, `\$`, `\#`, `\_`, `\{`, `\}`,
  `\\`, `\~`, `\^`) and the "The \& Song" rotation shape.
- `symbols` — titles with no word character (the `\#` block), `½` (word-ish
  but not alphabetic), `µ` (whose native uppercase is not valid UTF-8 and
  whose block character comes out as `Ÿ`), whitespace-stripping cases.
- `prefix` — the `%prefix ` directive (`\titleprefixword`), including one
  after the entries.
- `empty` — header only → empty output.
- `pass1-title`, `pass1-chord` — the actual `.sxd` bytes produced by a
  tectonic pass 1 over a book composed by `songbook-core::book` (same
  package files the browser engine loads), covering both index kinds
  end-to-end.

## Known native songidx 3.1 pathologies (not covered by goldens)

- Two entries whose sort reaches a word position held by a character that is
  `%w` but neither numeric nor alphabetic (e.g. `①`, or the `µ` garbage
  bytes) make native songidx loop forever; the port terminates instead.
- `%sep`/`%after`/`%ignore` directives crash native songidx (`wt_sep` etc.
  are never defined); the port returns a clear error.
- `AUTHOR`/`SCRIPTURE` index data files are not ported (the book pipeline
  never emits them); the port returns a clear error.
