FROM mcr.microsoft.com/devcontainers/rust:1-1-bookworm

ARG TECTONIC_VERSION=0.15.0
RUN curl -Ls "https://github.com/tectonic-typesetting/tectonic/releases/download/tectonic%40${TECTONIC_VERSION}/tectonic-${TECTONIC_VERSION}-x86_64-unknown-linux-musl.tar.gz" \
    | tar xz -C /usr/local/bin tectonic \
    && tectonic --version

# wasm-opt (binaryen 133, pinned + sha256) shrinks songbook_wasm_bg.wasm.
# Bookworm's binaryen 108 was broken here (mangled rustc 1.88 wasm); 133
# needs --enable-bulk-memory explicitly, and -all must NOT be used (its
# re-encoding is rejected by V8: "unknown import kind").
ARG BINARYEN_VERSION=133
RUN curl -fsSL "https://github.com/WebAssembly/binaryen/releases/download/version_${BINARYEN_VERSION}/binaryen-version_${BINARYEN_VERSION}-x86_64-linux.tar.gz" -o /tmp/binaryen.tar.gz \
    && echo "2dc9c7813f5375db93d96ead4b78222fcc3e2677bbb832297af4797782a37489  /tmp/binaryen.tar.gz" | sha256sum -c - \
    && tar xzf /tmp/binaryen.tar.gz -C /usr/local/bin --strip-components=2 "binaryen-version_${BINARYEN_VERSION}/bin/wasm-opt" \
    && rm /tmp/binaryen.tar.gz \
    && wasm-opt --version

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
# with, and an explicitly refreshed CA store keeps the pinned-upstream
# fetches (web/build-pdf-assets.sh) working across rotating CTAN mirrors.
RUN apt-get update \
    && apt-get install -y --no-install-recommends poppler-utils ca-certificates \
    && update-ca-certificates \
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
