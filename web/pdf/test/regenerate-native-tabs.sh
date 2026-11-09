#!/usr/bin/env bash
# Run inside songbook-devcontainer:latest, with texlua installed as documented
# in fixtures/README.md. PDFs/intermediates stay in the mounted scratch directory;
# extract committed references on the host with the smoke's own pdftotext.
set -euo pipefail
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/../../.." && pwd)"
W="${1:-/tmp/opencode/tab-integration/native}"
ASSETS="$REPO/web/dist/pdf-assets"
PINS="$REPO/web/pdf/pinned-hashes.txt"
command -v tectonic
command -v texlua
for component in engine/bundle.tar.gz pack/pack.tar.gz; do
  pin=pack.tar.gz
  if [[ "$component" == engine/* ]]; then pin=wasm.tectonic-bundle.tar.gz; fi
  want="$(awk -v name="$pin" '$1 == name { print $2 }' "$PINS")"
  [[ "$(sha256sum "$ASSETS/$component" | cut -d' ' -f1)" == "$want" ]]
done
[[ "$(sha256sum "$REPO/web/pdf/.cache/songs.zip" | cut -d' ' -f1)" == "$(awk '$1 == "songs.zip" { print $2 }' "$PINS")" ]]
mkdir -p "$W"
rm -rf "$W/bundle"
mkdir -p "$W/bundle"
tar -xzf "$ASSETS/engine/bundle.tar.gz" -C "$W/bundle"
tar -xzf "$ASSETS/pack/pack.tar.gz" -C "$W/bundle"
(
  cd "$W/bundle"
  find . -type f ! -name SHA256SUM | sed 's|^\./||' | LC_ALL=C sort | xargs sha256sum > "$W/SHA256SUM.tmp"
  mv "$W/SHA256SUM.tmp" SHA256SUM
)
unzip -p "$REPO/web/pdf/.cache/songs.zip" songs/songidx/songidx.lua > "$W/songidx.lua"
# Recompile historical pure chord/lyric baselines too; compare their extracted
# text without rewriting references to hide retention/default changes.
for fixture in let-it-be demo-book synthetic-tabs synthetic-tab-book \
  synthetic-annotated-tabs synthetic-annotated-tabs-split synthetic-annotated-tab-book; do
  rm -rf "$W/$fixture"
  mkdir -p "$W/$fixture"
  cp "$REPO/web/pdf/test/fixtures/$fixture.tex" "$W/$fixture/"
  (
    cd "$W/$fixture"
    tectonic -b "$W/bundle" -f latex.fmt --keep-intermediates "$fixture.tex"
    if [[ "$fixture" == *-book ]]; then
      LC_ALL=C texlua "$W/songidx.lua" titleidx.sxd titleidx.sbx
      tectonic -b "$W/bundle" -f latex.fmt --keep-intermediates "$fixture.tex"
    fi
  )
done
