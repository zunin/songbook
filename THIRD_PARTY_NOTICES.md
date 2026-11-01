# Third-party notices

This manifest describes the licensing of the Songbook browser release,
version 0.1.0. The current release is a private, browser-local editor that
produces LaTeX `.tex` source only. It does **not** contain a PDF engine or
produce PDF files.

## Songbook application

- Project: Songbook 0.1.0
- Copyright: Copyright (C) 2026 Nikolai Øllegaard
- License: GNU General Public License, version 3 or (at your option) any later
  version (GPL-3.0-or-later)
- Full license text: [LICENSE](LICENSE) in the source tree and
  [licenses/GPL-3.0.txt](licenses/GPL-3.0.txt)
- Corresponding source for a built release:
  [web/dist/songbook-source-0.1.0.tar.gz](web/dist/songbook-source-0.1.0.tar.gz)

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
`crates/songbook-wasm/Cargo.toml`, `Cargo.lock`, and the release
`wasm32-unknown-unknown` build artifacts.

| Component | Version | How it is shipped | License | Copyright / notice | Local license text | Source |
| --- | --- | --- | --- | --- | --- | --- |
| Songbook | 0.1.0 | Application HTML, CSS, JavaScript, and WebAssembly | GPL-3.0-or-later | Copyright (C) 2026 Nikolai Øllegaard | [GPL-3.0.txt](licenses/GPL-3.0.txt) | [web/dist/songbook-source-0.1.0.tar.gz](web/dist/songbook-source-0.1.0.tar.gz) |
| aho-corasick | 1.1.5 | Compiled into `songbook_wasm_bg.wasm` (via regex) | Unlicense OR MIT | Copyright (c) 2015 Andrew Gallant | [MIT text](licenses/MIT-aho-corasick-memchr.txt), [Unlicense](licenses/Unlicense.txt) | <https://crates.io/crates/aho-corasick/1.1.5> |
| anyhow | 1.0.104 | Compiled into `songbook_wasm_bg.wasm` (via songbook-core) | MIT OR Apache-2.0 | Upstream MIT and Apache-2.0 license files carry no separate copyright line | [MIT text](licenses/MIT-anyhow-once_cell-serde.txt), [Apache-2.0 text](licenses/Apache-2.0-no-appendix.txt) | <https://crates.io/crates/anyhow/1.0.104> |
| cfg-if | 1.0.5 | Compiled into `songbook_wasm_bg.wasm` (via wasm-bindgen) | MIT OR Apache-2.0 | Copyright (c) 2014 Alex Crichton | [MIT text](licenses/MIT-cfg-if-js-sys-wasm-bindgen.txt), [Apache-2.0 text](licenses/Apache-2.0.txt) | <https://crates.io/crates/cfg-if/1.0.5> |
| js-sys | 0.3.77 | Compiled into `songbook_wasm_bg.wasm` (via serde-wasm-bindgen) | MIT OR Apache-2.0 | Copyright (c) 2014 Alex Crichton | [MIT text](licenses/MIT-cfg-if-js-sys-wasm-bindgen.txt), [Apache-2.0 text](licenses/Apache-2.0.txt) | <https://crates.io/crates/js-sys/0.3.77> |
| memchr | 2.8.3 | Compiled into `songbook_wasm_bg.wasm` (via regex) | Unlicense OR MIT | Copyright (c) 2015 Andrew Gallant | [MIT text](licenses/MIT-aho-corasick-memchr.txt), [Unlicense](licenses/Unlicense.txt) | <https://crates.io/crates/memchr/2.8.3> |
| once_cell | 1.21.4 | Compiled into `songbook_wasm_bg.wasm` (via wasm-bindgen/js-sys) | MIT OR Apache-2.0 | Upstream MIT and Apache-2.0 license files carry no separate copyright line | [MIT text](licenses/MIT-anyhow-once_cell-serde.txt), [Apache-2.0 text](licenses/Apache-2.0.txt) | <https://crates.io/crates/once_cell/1.21.4> |
| regex | 1.13.1 | Compiled into `songbook_wasm_bg.wasm` (via songbook-core) | MIT OR Apache-2.0 | Copyright (c) 2014 The Rust Project Developers | [MIT text](licenses/MIT-regex.txt), [Apache-2.0 text](licenses/Apache-2.0.txt) | <https://crates.io/crates/regex/1.13.1> |
| regex-automata | 0.4.18 | Compiled into `songbook_wasm_bg.wasm` (via regex) | MIT OR Apache-2.0 | Copyright (c) 2014 The Rust Project Developers | [MIT text](licenses/MIT-regex.txt), [Apache-2.0 text](licenses/Apache-2.0.txt) | <https://crates.io/crates/regex-automata/0.4.18> |
| regex-syntax | 0.8.11 | Compiled into `songbook_wasm_bg.wasm` (via regex) | MIT OR Apache-2.0 | Copyright (c) 2014 The Rust Project Developers | [MIT text](licenses/MIT-regex.txt), [Apache-2.0 text](licenses/Apache-2.0.txt) | <https://crates.io/crates/regex-syntax/0.8.11> |
| serde | 1.0.229 | Compiled into `songbook_wasm_bg.wasm` (via songbook-wasm) | MIT OR Apache-2.0 | Upstream MIT and Apache-2.0 license files carry no separate copyright line | [MIT text](licenses/MIT-anyhow-once_cell-serde.txt), [Apache-2.0 text](licenses/Apache-2.0-no-appendix.txt) | <https://crates.io/crates/serde/1.0.229> |
| serde_core | 1.0.229 | Compiled into `songbook_wasm_bg.wasm` (via serde) | MIT OR Apache-2.0 | Upstream MIT and Apache-2.0 license files carry no separate copyright line | [MIT text](licenses/MIT-anyhow-once_cell-serde.txt), [Apache-2.0 text](licenses/Apache-2.0-no-appendix.txt) | <https://crates.io/crates/serde_core/1.0.229> |
| serde-wasm-bindgen | 0.6.5 | Compiled into `songbook_wasm_bg.wasm`; supports the JS/WASM boundary | MIT | Copyright (c) 2019 Cloudflare, Inc. | [MIT text](licenses/MIT-serde-wasm-bindgen.txt) | <https://crates.io/crates/serde-wasm-bindgen/0.6.5> |
| wasm-bindgen | 0.2.100 | Compiled into `songbook_wasm_bg.wasm`; generated JS glue is shipped as `songbook_wasm.js` | MIT OR Apache-2.0 | Copyright (c) 2014 Alex Crichton | [MIT text](licenses/MIT-cfg-if-js-sys-wasm-bindgen.txt), [Apache-2.0 text](licenses/Apache-2.0.txt) | <https://crates.io/crates/wasm-bindgen/0.2.100> |
| Rust standard library and compiler builtins | 1.88.0 | Linked into `songbook_wasm_bg.wasm` by the Rust toolchain | MIT OR Apache-2.0 | Copyright (c) The Rust Project Contributors | [MIT text](licenses/Rust-MIT.txt), [Apache-2.0 text](licenses/Rust-Apache-2.0.txt) | <https://github.com/rust-lang/rust/tree/1.88.0> |

No third-party JavaScript libraries, web fonts, images, or media files are
bundled in the current browser release. The application JavaScript, HTML, and
CSS are part of Songbook itself.

## Not bundled: planned PDF components

These components are **not present in the current `.tex`-only web release**.
They are recorded here for the planned PDF release and must be re-reviewed,
with their exact upstream sources and notices, before any PDF assets are
distributed.

| Component | Status in 0.1.0 web release | Planned role | License | Local license text | Source |
| --- | --- | --- | --- | --- | --- |
| GlyphTeX | NOT BUNDLED | Planned browser PDF engine | GPL-3.0 (assumed, using the least-permissive path) | [GPL-3.0.txt](licenses/GPL-3.0.txt) | Not bundled; record the exact distributed GlyphTeX source before enabling PDF output |
| songs.sty (LaTeX songs package) | NOT BUNDLED | Planned LaTeX package for PDF typesetting | GPL-2.0-or-later | [GPL-2.0.txt](licenses/GPL-2.0.txt) | <https://ctan.org/pkg/songs> |

The current `web/dist/` output contains no PDF engine, PDF output, PDF assets,
GlyphTeX files, or `songs.sty`.

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
official GNU license texts.
