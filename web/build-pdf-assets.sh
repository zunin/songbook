#!/usr/bin/env bash
# build-pdf-assets.sh — deterministic, offline-replayable build of the
# browser-local PDF engine assets (isolated engine/build boundary).
#
# Outputs (default web/dist/pdf-assets/, override with PDF_ASSETS_OUT=...):
#   engine/               GlyphTeX wasm + TeX bundle + ESM wrapper dist
#   pack/pack.tar.gz      songbook TeX pack: songs.sty (docstrip-generated),
#                         size12.clo, BaKoMa CM fonts + metrics
#   licenses/             carried license/notice files per component
#   sources/              full corresponding source (npm tgz, repo tarball,
#                         songs.dtx/.ins, classes.dtx/.ins)
#   (upstream download cache: web/pdf/.cache/ — outside the served tree)
#   manifest.json/md      per-component version + source URL + sha256 + license
#   docstrip-report.json
#   pdf-engine.js         deployed copy of web/pdf-engine.js
#   pdf-worker.js         deployed copy of web/pdf-worker.js
#   test/index.html       offline same-origin self-test page
#   test/fixtures/*       pinned standalone songs fixture + reference texts
#
# Scope: this script + web/pdf/ subtree (incl. the download cache at
# web/pdf/.cache/, Git-ignored via .gitignore) + web/dist/pdf-assets
# (Git-ignored via web/dist/).  Does NOT touch web/index.html, web/app.js,
# web/build.sh, LICENSE files, Cargo files, .gitignore, or docs; never commits.
#
# Usage:  bash web/build-pdf-assets.sh
#         FORCE_FETCH=1 bash web/build-pdf-assets.sh
#         PDF_ASSETS_OUT=/tmp/pdf-assets bash web/build-pdf-assets.sh
#         SMOKE_TEST=0 bash web/build-pdf-assets.sh   # skip node smoke test
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SRC="$REPO_ROOT/web"
PINS="$SRC/pdf/pinned-hashes.txt"
ASSETS="${PDF_ASSETS_OUT:-$SRC/dist/pdf-assets}"
CACHE="$SRC/pdf/.cache"
WORK="$ASSETS/build"
LICENSES="$ASSETS/licenses"
SOURCES="$ASSETS/sources"

GLYPHTEX_COMMIT="d4b44ac9d992297bf597de5bb0afa9b957eeac7c"
GLYPHTEX_COMMIT_DATE="2026-10-04"

say() { printf '%s\n' "$*"; }
hash_of() { sha256sum "$1" | cut -d' ' -f1; }
pin_of() { awk -v n="$1" '$1 == n {print $2}' "$PINS"; }

# fetch <cachename> <url> <pinname>
fetch() {
  local name="$1" url="$2" pin="$3"
  local want; want="$(pin_of "$pin")"
  local out="$CACHE/$name"
  if [[ -f "$out" && -z "${FORCE_FETCH:-}" ]]; then
    say "cache: $name (cached)"
  else
    say "fetch: $name from $url"
    curl -fsSL --retry 3 -o "$out" "$url"
  fi
  local got; got="$(hash_of "$out")"
  if [[ "$got" != "$want" ]]; then
    say "FATAL: $name sha256 mismatch (got $got, want $want)" >&2
    exit 1
  fi
  say "  sha256 OK: $got"
}

say "=== PDF engine asset build ==="
say "output: $ASSETS"
mkdir -p "$CACHE" "$WORK" "$LICENSES" "$SOURCES" "$ASSETS/engine/generated" "$ASSETS/pack" "$ASSETS/test/fixtures"

# --- 1. pinned upstream sources --------------------------------------------
fetch "glyphtex-engine.tgz" \
  "https://registry.npmjs.org/glyphtex-engine/-/glyphtex-engine-0.1.0.tgz" \
  "glyphtex-engine.tgz"
fetch "glyphtex-repo.tar.gz" \
  "https://github.com/kanakkholwal/glyphtex/archive/$GLYPHTEX_COMMIT.tar.gz" \
  "glyphtex-repo.tar.gz"
fetch "songs.zip" \
  "https://mirrors.ctan.org/macros/latex/contrib/songs.zip" "songs.zip"
fetch "classes.dtx" \
  "https://mirrors.ctan.org/macros/latex/base/classes.dtx" "classes.dtx"
fetch "classes.ins" \
  "https://mirrors.ctan.org/macros/latex/base/classes.ins" "classes.ins"
fetch "bakoma.zip" \
  "https://mirrors.ctan.org/fonts/cm/ps-type1/bakoma.zip" "bakoma.zip"
fetch "LICENSE-glyphtex" \
  "https://raw.githubusercontent.com/kanakkholwal/glyphtex/$GLYPHTEX_COMMIT/LICENSE" \
  "LICENSE-glyphtex"

# --- 2. unpack --------------------------------------------------------------
say "unpacking"
rm -rf "$WORK/engine-pkg" "$WORK/songs" "$WORK/bakoma" "$WORK/pack-files"
mkdir -p "$WORK/engine-pkg" "$WORK/songs" "$WORK/bakoma" "$WORK/pack-files"
tar -xzf "$CACHE/glyphtex-engine.tgz" -C "$WORK/engine-pkg" --strip-components=1
unzip -q "$CACHE/songs.zip" -d "$WORK/songs"
unzip -q "$CACHE/bakoma.zip" -d "$WORK/bakoma"

# engine wasm + bundle hashes (from vendored tarball, verified vs pins)
cp "$WORK/engine-pkg/wasm/tectonic_wasm.wasm" "$ASSETS/engine/tectonic.wasm"
cp "$WORK/engine-pkg/wasm/tectonic-bundle.tar.gz" "$ASSETS/engine/bundle.tar.gz"
[[ "$(hash_of "$ASSETS/engine/tectonic.wasm")" == "$(pin_of wasm.tectonic.wasm)" ]] || { say "FATAL: wasm hash mismatch" >&2; exit 1; }
[[ "$(hash_of "$ASSETS/engine/bundle.tar.gz")" == "$(pin_of wasm.tectonic-bundle.tar.gz)" ]] || { say "FATAL: bundle hash mismatch" >&2; exit 1; }

# engine ESM runtime (dist/*, minus source maps — wrapper is small)
cp "$WORK/engine-pkg/dist/index.js"    "$ASSETS/engine/index.js"
cp "$WORK/engine-pkg/dist/engine.js"   "$ASSETS/engine/engine.js"
cp "$WORK/engine-pkg/dist/imports.js"  "$ASSETS/engine/imports.js"
cp "$WORK/engine-pkg/dist/packs.js"    "$ASSETS/engine/packs.js"
cp "$WORK/engine-pkg/dist/generated/"* "$ASSETS/engine/generated/" 2>/dev/null || true

# --- 3. docstrip generation (verified) ---------------------------------------
say "docstrip: generating songs.sty and size12.clo from pinned sources"
cat > "$WORK/docstrip-job.json" <<EOF
{
  "sources": {
    "songs.dtx":   {"path": "$WORK/songs/songs/songs.dtx"},
    "songs.ins":   {"path": "$WORK/songs/songs/songs.ins"},
    "classes.dtx": {"path": "$CACHE/classes.dtx"},
    "classes.ins": {"path": "$CACHE/classes.ins"}
  },
  "jobs": [
    {"output": "songs.sty", "dtx": "songs.dtx", "ins": "songs.ins",
     "module": "package", "expect_sha256": "$(pin_of docstrip.songs.sty)"},
    {"output": "size12.clo", "dtx": "classes.dtx", "ins": "classes.ins",
     "module": "12pt", "expect_sha256": "$(pin_of docstrip.size12.clo)"}
  ],
  "report": "$ASSETS/docstrip-report.json"
}
EOF
# docstrip.py matches "output" against the basenames declared in the .ins
# and opens it relative to the process CWD, so run it from pack-files
# (the "report" path stays absolute and is CWD-independent).
( cd "$WORK/pack-files" && python3 "$SRC/pdf/tools/docstrip.py" "$WORK/docstrip-job.json" )

# --- 4. font pack ------------------------------------------------------------
say "assembling pack (BaKoMa CM fonts + metrics + generated sty/clo)"
# Pack members must be flat bare names: the engine's MemoryIo looks files up
# by exact addFile() key (HashMap get, no directory handling — io.rs) and
# kpathsea requests TeX inputs by bare name, exactly like the engine bundle
# itself (article.cls, cmr10.tfm, ... are all flat members).
cp "$WORK/bakoma/bakoma/tfm/"*.tfm "$WORK/pack-files/"
cp "$WORK/bakoma/bakoma/pfb/"*.pfb "$WORK/pack-files/"

# deterministic pack tarball (flat members, no "./" prefix, fixed metadata;
# LC_ALL=C pins glob order, --sort=name orders any directory traversal)
( cd "$WORK/pack-files" && LC_ALL=C tar --sort=name --mtime='UTC 1970-01-01' \
    --owner=0 --group=0 --numeric-owner \
    -czf "$ASSETS/pack/pack.tar.gz" songs.sty size12.clo *.tfm *.pfb )
# pinned pack bytes (web/pdf/pinned-hashes.txt): determinism as a build-time
# fitness check — a deliberate pack change must update the pin.
[[ "$(hash_of "$ASSETS/pack/pack.tar.gz")" == "$(pin_of pack.tar.gz)" ]] || { say "FATAL: pack.tar.gz hash mismatch" >&2; exit 1; }

# --- 5. licenses + sources ----------------------------------------------------
say "carrying licenses and sources"
cp "$CACHE/LICENSE-glyphtex" "$LICENSES/LICENSE.glyphtex"
cp "$WORK/songs/songs/license.txt" "$LICENSES/LICENSE.songs.txt"
cp "$CACHE/classes.ins" "$SOURCES/classes.ins"
cp "$CACHE/classes.dtx" "$SOURCES/classes.dtx"
cp "$WORK/bakoma/bakoma/LICENCE" "$LICENSES/LICENSE.bakoma.txt"
cp "$CACHE/glyphtex-engine.tgz" "$SOURCES/"
cp "$CACHE/glyphtex-repo.tar.gz" "$SOURCES/"
cp "$CACHE/songs.zip" "$SOURCES/"
cp "$CACHE/bakoma.zip" "$SOURCES/"

# --- 6. manifest ---------------------------------------------------------------
say "writing manifest"
python3 - "$ASSETS" "$GLYPHTEX_COMMIT" "$GLYPHTEX_COMMIT_DATE" "$CACHE" <<'PYEOF'
import hashlib, json, os, sys
assets, commit, commit_date, cache = sys.argv[1], sys.argv[2], sys.argv[3], sys.argv[4]
def sha(p):
    h = hashlib.sha256()
    with open(p, "rb") as f:
        for c in iter(lambda: f.read(1 << 16), b""): h.update(c)
    return h.hexdigest()
def pack_info():
    p = os.path.join(assets, "pack", "pack.tar.gz")
    n = 0
    with tarfile.open(p) as tf: n = len(tf.getmembers())
    return {"files": n, "sha256": sha(p), "bytes": os.path.getsize(p)}
import tarfile
components = [
 {"id": "glyphtex-engine", "version": "0.1.0",
  "source_url": "https://registry.npmjs.org/glyphtex-engine/-/glyphtex-engine-0.1.0.tgz",
  "source_sha256": sha(f"{assets}/sources/glyphtex-engine.tgz"),
  "license": "GPL-3.0 (conservative assumption)",
  "license_note": "npm package.json declares MIT, but the upstream repo kanakkholwal/glyphtex is GPL-3.0 (LICENSE carried from the repo at commit). Operative license per project decision: GPL-3.0. Do not ship under MIT without a written upstream grant.",
  "runtime_assets": ["engine/tectonic.wasm", "engine/bundle.tar.gz", "engine/*.js"]},
 {"id": "glyphtex-repo", "version": f"commit {commit} ({commit_date})",
  "source_url": f"https://github.com/kanakkholwal/glyphtex/archive/{commit}.tar.gz",
  "source_sha256": sha(f"{assets}/sources/glyphtex-repo.tar.gz"),
  "license": "GPL-3.0", "license_note": "Complete corresponding source for the wasm build, carried to satisfy GPL source obligations."},
 {"id": "songs", "version": "3.1 (2018/09/12)",
  "source_url": "https://mirrors.ctan.org/macros/latex/contrib/songs.zip",
  "source_sha256": sha(f"{assets}/sources/songs.zip"),
  "license": "GPL-2-or-later (CTAN songs package, Kevin W. Hamlen)",
  "license_note": "songs.sty is docstrip-generated from pinned songs.dtx/.ins (source carried). License file carried."},
 {"id": "classes", "version": "2026-06-04 v1.4n (LaTeX base)",
  "source_url": "https://mirrors.ctan.org/macros/latex/base/classes.dtx",
  "source_sha256": sha(f"{cache}/classes.dtx"),
  "license": "LPPL 1.3c",
  "license_note": "size12.clo is docstrip-generated from pinned classes.dtx/.ins (source carried)."},
 {"id": "bakoma", "version": "1.1/12-Nov-94 CM + 1.2/19-Jan-95 AMS",
  "source_url": "https://mirrors.ctan.org/fonts/cm/ps-type1/bakoma.zip",
  "source_sha256": sha(f"{assets}/sources/bakoma.zip"),
  "license": "BaKoMa Fonts Licence (free redistribution, notice retention required, PDF embedding allowed)",
  "license_note": "LICENCE file carried."},
]
manifest = {"built_by": "web/build-pdf-assets.sh", "components": components,
            "pack": pack_info(), "policy": {"song_content_in_assets": False}}
with open(f"{assets}/manifest.json", "w") as f: json.dump(manifest, f, indent=2)
PYEOF

# --- 7. deploy module + offline test page -------------------------------------
say "deploying module + test page"
# Node's module resolver needs a "type": "module" marker: the asset tree has
# no package.json of its own and every .js here is an ES module (the engine's
# ESM dist and pdf-worker.js). Browsers ignore this file; it exists so the
# smoke test's node can import the tree the way the browser does.
printf '{"type": "module"}\n' > "$ASSETS/package.json"
cp "$SRC/pdf-engine.js" "$ASSETS/pdf-engine.js"
cp "$SRC/pdf-worker.js" "$ASSETS/pdf-worker.js"
cp "$SRC/pdf/test/index.html" "$ASSETS/test/index.html"
cp "$SRC/pdf/test/fixtures/let-it-be.tex" "$ASSETS/test/fixtures/"
cp "$SRC/pdf/test/fixtures/let-it-be-reference.txt" "$ASSETS/test/fixtures/"
cp "$SRC/pdf/test/fixtures/let-it-be-wasm.txt" "$ASSETS/test/fixtures/"
cp "$SRC/pdf/pinned-hashes.txt" "$ASSETS/pinned-hashes.txt"

# --- 8. optional node smoke test -----------------------------------------------
if [[ "${SMOKE_TEST:-1}" == "1" ]]; then
  say "node smoke test (fixture compile via assembled engine + pack)"
  node "$SRC/pdf/test/smoke-node.mjs" "$ASSETS" "$SRC/pdf/test/fixtures"
fi

say "build complete: $ASSETS"
