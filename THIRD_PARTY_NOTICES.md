# Third-party notices

This manifest describes the licensing of the Songbook browser release,
version 0.1.0. The current release is a private, browser-local editor that
produces LaTeX `.tex` source and compiles PDF files locally in the browser.
The PDF engine and TeX assets are built by
[web/build-pdf-assets.sh](web/build-pdf-assets.sh) from hash-pinned upstream
sources into `web/dist/pdf-assets/` and are lazy-loaded: nothing PDF-related
is fetched until the user first requests a PDF.

## Songbook application

- Project: Songbook 0.1.0
- Copyright: Copyright (C) 2026 @nikolaioellegaard
- License: GNU General Public License, version 3 or (at your option) any later
  version (GPL-3.0-or-later)
- Full license text: [LICENSE](LICENSE) in the source tree and
  [licenses/GPL-3.0.txt](licenses/GPL-3.0.txt)
- Corresponding source for a built release: the versioned source archive is
  provided alongside the browser assets; build it from committed sources with
  [web/source-bundle.sh](web/source-bundle.sh).

Songbook is free software: you can redistribute it and/or modify it under the
terms of the GNU General Public License as published by the Free Software
Foundation, either version 3 of the License, or (at your option) any later
version.

Songbook is distributed in the hope that it will be useful, but WITHOUT ANY
WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS FOR A
PARTICULAR PURPOSE. See the GNU General Public License for more details.

## Components shipped in the current browser release

The following components are compiled into or distributed with the current
`web/dist/` browser assets. License texts are reproduced verbatim in
[`licenses/`](licenses/). The shipped set was determined from
`crates/songbook-wasm/Cargo.toml`, `Cargo.lock`, the release
`wasm32-unknown-unknown` build artifacts, and, for the PDF assets,
`web/dist/pdf-assets/manifest.json` (built from the pinned hashes recorded in
`web/pdf/pinned-hashes.txt`).

| Component | Version | How it is shipped | License | Copyright / notice | Local license text | Source |
| --- | --- | --- | --- | --- | --- | --- |
| Songbook | 0.1.0 | Application HTML, CSS, JavaScript, and WebAssembly | GPL-3.0-or-later | Copyright (C) 2026 @nikolaioellegaard | [GPL-3.0.txt](licenses/GPL-3.0.txt) | Source for this release is provided in the versioned source archive. |
| aho-corasick | 1.1.5 | Compiled into `songbook_wasm_bg.wasm` (via regex) | Unlicense OR MIT | Copyright (c) 2015 Andrew Gallant | [MIT text](licenses/MIT-aho-corasick-memchr.txt), [Unlicense](licenses/Unlicense.txt) | <https://crates.io/crates/aho-corasick/1.1.5> |
| anyhow | 1.0.104 | Compiled into `songbook_wasm_bg.wasm` (via songbook-core) | MIT OR Apache-2.0 | Upstream MIT and Apache-2.0 license files carry no separate copyright line | [MIT text](licenses/MIT-anyhow-once_cell-serde.txt), [Apache-2.0 text](licenses/Apache-2.0-no-appendix.txt) | <https://crates.io/crates/anyhow/1.0.104> |
| BaKoMa CM/AMS fonts | 1.1/12-Nov-94 CM + 1.2/19-Jan-95 AMS | Type 1/TrueType font files in the `pdf-assets/` TeX pack, fetched lazily on first PDF output | BaKoMa Fonts Licence (free redistribution, notice retention required, PDF embedding allowed) | Copyright (C) 1994, 1995, Basil K. Malyshev. All Rights Reserved. | [BaKoMa.txt](licenses/BaKoMa.txt), also carried as `pdf-assets/licenses/LICENSE.bakoma.txt` | <https://mirrors.ctan.org/fonts/cm/ps-type1/bakoma.zip> (sha256 6fb67f2fdaf5846fe81734d04d69096b847d2b41ca6da7923a9a73437e588734); pinned sources and the build manifest ship with the assets (`pdf-assets/manifest.json`, `pdf-assets/sources/`). |
| cfg-if | 1.0.5 | Compiled into `songbook_wasm_bg.wasm` (via wasm-bindgen) | MIT OR Apache-2.0 | Copyright (c) 2014 Alex Crichton | [MIT text](licenses/MIT-cfg-if-js-sys-wasm-bindgen.txt), [Apache-2.0 text](licenses/Apache-2.0.txt) | <https://crates.io/crates/cfg-if/1.0.5> |
| classes / size12.clo (LaTeX base) | 2026-06-04 v1.4n (LaTeX base) | `size12.clo` in the `pdf-assets/` TeX pack, docstrip-generated from pinned `classes.dtx`/`classes.ins`, fetched lazily on first PDF output | LPPL 1.3c | Copyright (C) 1993-2026 The LaTeX Project and any individual authors listed elsewhere in the file | No verbatim license file carried; LPPL 1.3c text at <https://www.latex-project.org/lppl/lppl-1-3c.txt> | <https://mirrors.ctan.org/macros/latex/base/classes.dtx> (sha256 7d2add3d0c5f4deff4aaae06e61c0332aef89b19e56be26c105cb8bc34189a41); docstrip sources (`classes.dtx`, `classes.ins`) are carried in `pdf-assets/sources/` (see `pdf-assets/manifest.json`). |
| glyphtex-engine (Tectonic WASM engine) | 0.1.0 | `engine/tectonic.wasm`, `engine/bundle.tar.gz`, and engine JavaScript in `pdf-assets/`, fetched lazily on first PDF output | GPL-3.0 (operative; conservative project decision — npm package metadata declares MIT) | Upstream GPL-3.0 `LICENSE` carried from the `kanakkholwal/glyphtex` repo at the pinned commit; do not ship under MIT without a written upstream grant | [GPL-3.0.txt](licenses/GPL-3.0.txt); upstream copy carried as `pdf-assets/licenses/LICENSE.glyphtex` | <https://registry.npmjs.org/glyphtex-engine/-/glyphtex-engine-0.1.0.tgz> (sha256 66498cff5d1c9338f07624630837def8344bf7570e37599d3c7b1a1f9fdae55a); complete corresponding source carried in `pdf-assets/sources/` (see `pdf-assets/manifest.json`). |
| glyphtex (upstream repo source) | commit d4b44ac9d992297bf597de5bb0afa9b957eeac7c (2026-10-04) | Complete corresponding source for the WASM engine build, carried as `pdf-assets/sources/glyphtex-repo.tar.gz` | GPL-3.0 | Upstream GPL-3.0 `LICENSE` carried at the pinned commit | [GPL-3.0.txt](licenses/GPL-3.0.txt); upstream copy carried as `pdf-assets/licenses/LICENSE.glyphtex` | <https://github.com/kanakkholwal/glyphtex/archive/d4b44ac9d992297bf597de5bb0afa9b957eeac7c.tar.gz> (sha256 db12bec7ee7531e9c52dd3a7f2f6b0718b1dbd6fa42363ef59d9386d71bed687); see `pdf-assets/manifest.json`. |
| js-sys | 0.3.77 | Compiled into `songbook_wasm_bg.wasm` (via serde-wasm-bindgen) | MIT OR Apache-2.0 | Copyright (c) 2014 Alex Crichton | [MIT text](licenses/MIT-cfg-if-js-sys-wasm-bindgen.txt), [Apache-2.0 text](licenses/Apache-2.0.txt) | <https://crates.io/crates/js-sys/0.3.77> |
| memchr | 2.8.3 | Compiled into `songbook_wasm_bg.wasm` (via regex) | Unlicense OR MIT | Copyright (c) 2015 Andrew Gallant | [MIT text](licenses/MIT-aho-corasick-memchr.txt), [Unlicense](licenses/Unlicense.txt) | <https://crates.io/crates/memchr/2.8.3> |
| once_cell | 1.21.4 | Compiled into `songbook_wasm_bg.wasm` (via wasm-bindgen/js-sys) | MIT OR Apache-2.0 | Upstream MIT and Apache-2.0 license files carry no separate copyright line | [MIT text](licenses/MIT-anyhow-once_cell-serde.txt), [Apache-2.0 text](licenses/Apache-2.0.txt) | <https://crates.io/crates/once_cell/1.21.4> |
| regex | 1.13.1 | Compiled into `songbook_wasm_bg.wasm` (via songbook-core) | MIT OR Apache-2.0 | Copyright (c) 2014 The Rust Project Developers | [MIT text](licenses/MIT-regex.txt), [Apache-2.0 text](licenses/Apache-2.0.txt) | <https://crates.io/crates/regex/1.13.1> |
| regex-automata | 0.4.18 | Compiled into `songbook_wasm_bg.wasm` (via regex) | MIT OR Apache-2.0 | Copyright (c) 2014 The Rust Project Developers | [MIT text](licenses/MIT-regex.txt), [Apache-2.0 text](licenses/Apache-2.0.txt) | <https://crates.io/crates/regex-automata/0.4.18> |
| regex-syntax | 0.8.11 | Compiled into `songbook_wasm_bg.wasm` (via regex) | MIT OR Apache-2.0 | Copyright (c) 2014 The Rust Project Developers | [MIT text](licenses/MIT-regex.txt), [Apache-2.0 text](licenses/Apache-2.0.txt) | <https://crates.io/crates/regex-syntax/0.8.11> |
| serde | 1.0.229 | Compiled into `songbook_wasm_bg.wasm` (via songbook-wasm) | MIT OR Apache-2.0 | Upstream MIT and Apache-2.0 license files carry no separate copyright line | [MIT text](licenses/MIT-anyhow-once_cell-serde.txt), [Apache-2.0 text](licenses/Apache-2.0-no-appendix.txt) | <https://crates.io/crates/serde/1.0.229> |
| serde_core | 1.0.229 | Compiled into `songbook_wasm_bg.wasm` (via serde) | MIT OR Apache-2.0 | Upstream MIT and Apache-2.0 license files carry no separate copyright line | [MIT text](licenses/MIT-anyhow-once_cell-serde.txt), [Apache-2.0 text](licenses/Apache-2.0-no-appendix.txt) | <https://crates.io/crates/serde_core/1.0.229> |
| serde-wasm-bindgen | 0.6.5 | Compiled into `songbook_wasm_bg.wasm`; supports the JS/WASM boundary | MIT | Copyright (c) 2019 Cloudflare, Inc. | [MIT text](licenses/MIT-serde-wasm-bindgen.txt) | <https://crates.io/crates/serde-wasm-bindgen/0.6.5> |
| songs (LaTeX songs package) | 3.1 (2018/09/12) | `songs.sty` in the `pdf-assets/` TeX pack, docstrip-generated from pinned `songs.dtx`/`songs.ins`, fetched lazily on first PDF output | GPL-2.0-or-later | Copyright (C) 2018 Kevin W. Hamlen | [GPL-2.0.txt](licenses/GPL-2.0.txt); upstream copy carried as `pdf-assets/licenses/LICENSE.songs.txt` | <https://mirrors.ctan.org/macros/latex/contrib/songs.zip> (sha256 9449fc00c63df3af5e4bf01a7c54badced85d7286c86e8a9ee90b5c51b2d1c77); docstrip sources are carried in `pdf-assets/sources/` (see `pdf-assets/manifest.json`). |
| wasm-bindgen | 0.2.100 | Compiled into `songbook_wasm_bg.wasm`; generated JS glue is shipped as `songbook_wasm.js` | MIT OR Apache-2.0 | Copyright (c) 2014 Alex Crichton | [MIT text](licenses/MIT-cfg-if-js-sys-wasm-bindgen.txt), [Apache-2.0 text](licenses/Apache-2.0.txt) | <https://crates.io/crates/wasm-bindgen/0.2.100> |
| Rust standard library and compiler builtins | 1.88.0 | Linked into `songbook_wasm_bg.wasm` by the Rust toolchain | MIT OR Apache-2.0 | Copyright (c) The Rust Project Contributors | [MIT text](licenses/Rust-MIT.txt), [Apache-2.0 text](licenses/Rust-Apache-2.0.txt) | <https://github.com/rust-lang/rust/tree/1.88.0> |

No third-party JavaScript libraries, images, or media files are bundled in the
current browser release. The application JavaScript, HTML, and CSS are part of
Songbook itself. The PDF engine and TeX components listed above are shipped
only as `web/dist/pdf-assets/` and are loaded lazily — nothing PDF-related is
fetched until the user first requests a PDF, and the assets contain no song
content.

## PDF assets

The PDF engine and TeX components are bundled as `web/dist/pdf-assets/`, built
only by [web/build-pdf-assets.sh](web/build-pdf-assets.sh) from hash-pinned
upstream sources. Their authoritative component record is
`pdf-assets/manifest.json`, which ships with the assets together with the
complete corresponding sources (`pdf-assets/sources/`) and the upstream license
files (`pdf-assets/licenses/`). No PDF assets ship without their manifest and
sources.

## Not part of this release

`Cargo.lock` also contains dependencies for the command-line and TUI programs
(for example clap, crossterm, ratatui, and their transitive dependencies), as
well as build-time-only proc-macro crates. Those crates are not compiled into
or distributed with the current browser WASM assets and are therefore not
listed as shipped browser components.

## License text notes

The files under [`licenses/`](licenses/) are verbatim copies of the license
files shipped by the corresponding upstream projects, except that their names
have been made descriptive. The two Apache-2.0 copies differ only in whether
the upstream file includes the optional "How to apply" appendix. The MIT copies
differ only in their upstream copyright lines. GPL-3.0 and GPL-2.0 are the
official GNU license texts. `BaKoMa.txt` is the verbatim upstream `LICENCE`
file from the BaKoMa font pack. The PDF assets additionally carry verbatim
upstream license files under `pdf-assets/licenses/` and their pinned upstream
sources under `pdf-assets/sources/`.
