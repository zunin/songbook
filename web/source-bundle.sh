#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "$0")/.." && pwd)"
cd "$repo_root"

if ! git diff --quiet --ignore-submodules -- || ! git diff --cached --quiet --ignore-submodules --; then
  echo "error: tracked files have uncommitted changes; commit or stash them first" >&2
  exit 1
fi

version="$(sed -n 's/^version *= *"\([^"]*\)".*/\1/p' Cargo.toml | head -n 1)"
if [ -z "$version" ]; then
  echo "error: could not read package version from Cargo.toml" >&2
  exit 1
fi

out_dir="web/dist"
if ! git check-ignore -q "$out_dir"; then
  echo "error: $out_dir is not ignored; refusing to write the bundle there" >&2
  exit 1
fi

bundle_name="songbook-${version}"
archive="${out_dir}/songbook-source-${version}.tar.gz"

mkdir -p "$out_dir"
git archive --format=tar.gz --prefix="${bundle_name}/" --output="$archive" HEAD

echo "created $archive"
