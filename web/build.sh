#!/usr/bin/env bash
# Repeatable local build for the browser MVP:
#   1. compile songbook-wasm for wasm32-unknown-unknown (release)
#   2. generate wasm-bindgen JS glue pinned to the crate's wasm-bindgen version
#   3. assemble the static app into web/dist/
#
# Requirements: cargo with the wasm32-unknown-unknown target, and
# wasm-bindgen-cli whose version matches the wasm-bindgen crate version
# (pinned in crates/songbook-wasm/Cargo.toml). Run inside the devcontainer.
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

wasm_target="wasm32-unknown-unknown"
out_dir="web/dist"

# Pin check: extract "=X.Y.Z" from [target.'cfg(target_arch = "wasm32")'.dependencies]
pin="$(sed -n 's/^wasm-bindgen *= *"\(=.*\)"/\1/p' crates/songbook-wasm/Cargo.toml | tr -d '=')"
if [ -z "$pin" ]; then
  echo "error: could not find pinned wasm-bindgen version in crates/songbook-wasm/Cargo.toml" >&2
  exit 1
fi

if ! command -v wasm-bindgen >/dev/null 2>&1; then
  echo "error: wasm-bindgen-cli not found." >&2
  echo "install the matching pinned version inside the devcontainer:" >&2
  echo "  cargo install wasm-bindgen-cli --version $pin --locked" >&2
  exit 1
fi

installed="$(wasm-bindgen --version | awk '{print $2}')"
if [ "$installed" != "$pin" ]; then
  echo "error: wasm-bindgen-cli version mismatch: have $installed, crate pins $pin" >&2
  echo "install the matching pinned version inside the devcontainer:" >&2
  echo "  cargo install wasm-bindgen-cli --version $pin --locked" >&2
  exit 1
fi

if ! rustup target list --installed 2>/dev/null | grep -qx "$wasm_target"; then
  echo "error: rust target $wasm_target is not installed." >&2
  echo "install it inside the devcontainer:  rustup target add $wasm_target" >&2
  exit 1
fi

echo "==> cargo build -p songbook-wasm --target $wasm_target --release"
cargo build -p songbook-wasm --target "$wasm_target" --release

wasm_path="target/$wasm_target/release/songbook_wasm.wasm"

echo "==> wasm-bindgen (target web) -> $out_dir"
rm -rf "$out_dir"
mkdir -p "$out_dir"
wasm-bindgen --target web --out-dir "$out_dir" --out-name songbook_wasm "$wasm_path"

# Node's module resolver needs a "type": "module" marker: dist has no
# package.json and the glue (songbook_wasm.js) is an ES module. Browsers
# ignore this file; it lets the PDF worker's lazy glue import work under
# node (the smoke) exactly as it does in the browser.
printf '{"type": "module"}\n' > "$out_dir/package.json"

if command -v wasm-opt >/dev/null 2>&1; then
  echo "==> wasm-opt -Oz"
  wasm-opt -Oz "$out_dir/songbook_wasm_bg.wasm" -o "$out_dir/songbook_wasm_bg.wasm"
else
  echo "==> wasm-opt not available; skipping size optimization"
fi

echo "==> copy static and license assets -> $out_dir"
cp web/index.html web/styles.css web/app.js web/licenses.html "$out_dir/"
cp LICENSE THIRD_PARTY_NOTICES.md "$out_dir/"
cp -R licenses "$out_dir/licenses"
./web/source-bundle.sh

echo "==> done: serve the folder, e.g.  python3 -m http.server -d web/dist 8080"
ls -l "$out_dir"
