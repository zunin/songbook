FROM mcr.microsoft.com/devcontainers/rust:1-1-bookworm

ARG TECTONIC_VERSION=0.15.0
RUN curl -Ls "https://github.com/tectonic-typesetting/tectonic/releases/download/tectonic%40${TECTONIC_VERSION}/tectonic-${TECTONIC_VERSION}-x86_64-unknown-linux-musl.tar.gz" \
    | tar xz -C /usr/local/bin tectonic \
    && tectonic --version

# NOTE: wasm-opt (binaryen) is deliberately NOT installed here. Tested on
# 2026-11-01 against this project's rustc 1.88 / wasm-bindgen 0.2.100 output:
# - Debian bookworm's binaryen 108 rejects the module outright (bulk memory /
#   sign-ext), and its flag-enabled output was also rejected by V8;
# - pinned upstream binaryen 133 (--all -Oz) validated and optimized the
#   module (1,554,257 -> 1,414,919 B) but the RESULT was rejected by V8 at
#   instantiation ("unknown import kind 0x7f") under node 26 — while the
#   unoptimized module instantiates fine everywhere.
# Do not re-enable without a newer binaryen AND the full smoke gate (both
# fixtures must pass byte-exact on the optimized wasm).

# web/build.sh requires the wasm32 rust target and a wasm-bindgen-cli whose
# version exactly matches the `wasm-bindgen = "=0.2.100"` pin in
# crates/songbook-wasm/Cargo.toml (it hard-fails on any mismatch), so keep
# WASM_BINDGEN_VERSION in sync with that pin.
ARG WASM_BINDGEN_VERSION=0.2.100
# Install as the image's `vscode` user so the wasm target and CLI land in the
# shared RUSTUP_HOME/CARGO_HOME (/usr/local/rustup, /usr/local/cargo) that is
# on the PATH of the user who runs builds.
USER vscode
RUN rustup target add wasm32-unknown-unknown \
    && rustup target list --installed | grep -x wasm32-unknown-unknown \
    && cargo install wasm-bindgen-cli --version "${WASM_BINDGEN_VERSION}" --locked \
    && wasm-bindgen --version
USER root

# poppler-utils provides the pdftotext/pdfinfo that the PDF smoke compares
# with. (Node itself is installed from a pinned tarball below.)
RUN apt-get update \
    && apt-get install -y --no-install-recommends poppler-utils \
    && rm -rf /var/lib/apt/lists/* \
    && pdftotext -v 2>&1 | head -1 && pdfinfo -v 2>&1 | head -1

# Node >= 26 is required to instantiate the songbook wasm: rustc 1.88 emits
# wasm value types (typed function references) that node 18/22's V8 reject
# at validation. The project owner's browser and host node 26 accept them,
# so pin node 26 (LTS since 2026-10). House style: pinned version + sha256,
# like tectonic above.
ARG NODE_VERSION=26.13.0
RUN curl -fsSL "https://nodejs.org/dist/v${NODE_VERSION}/node-v${NODE_VERSION}-linux-x64.tar.xz" -o /tmp/node.tar.xz \
    && echo "d28a96af6c38596f6d9d97c0ee531c13d238d757742387d1928de822d959e997  /tmp/node.tar.xz" | sha256sum -c - \
    && tar -xJf /tmp/node.tar.xz -C /usr/local --strip-components=1 --no-same-owner \
    && rm /tmp/node.tar.xz \
    && node --version && pdftotext -v 2>&1 | head -1
