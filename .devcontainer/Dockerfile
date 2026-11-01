FROM mcr.microsoft.com/devcontainers/rust:1-1-bookworm

ARG TECTONIC_VERSION=0.15.0
RUN curl -Ls "https://github.com/tectonic-typesetting/tectonic/releases/download/tectonic%40${TECTONIC_VERSION}/tectonic-${TECTONIC_VERSION}-x86_64-unknown-linux-musl.tar.gz" \
    | tar xz -C /usr/local/bin tectonic \
    && tectonic --version

# wasm-opt, used optionally by web/build.sh for size optimization.
RUN apt-get update \
    && apt-get install -y --no-install-recommends binaryen \
    && rm -rf /var/lib/apt/lists/* \
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

# Node.js runs the PDF asset build's smoke test (the default final step of
# web/build-pdf-assets.sh) and the external PDF smoke; Debian bookworm's
# nodejs covers the plain-stdlib usage in web/pdf/test/smoke-node.mjs, and
# poppler-utils provides the pdftotext/pdfinfo that smoke compares with.
RUN apt-get update \
    && apt-get install -y --no-install-recommends nodejs poppler-utils \
    && rm -rf /var/lib/apt/lists/* \
    && node --version && pdftotext -v 2>&1 | head -1 && pdfinfo -v 2>&1 | head -1
