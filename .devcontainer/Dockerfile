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

# poppler-utils provides the pdftotext/pdfinfo that the PDF smoke compares
# with. (Node itself is installed from a pinned tarball below.)
RUN apt-get update \
    && apt-get install -y --no-install-recommends poppler-utils \
    && rm -rf /var/lib/apt/lists/* \
    && pdftotext -v 2>&1 | head -1 && pdfinfo -v 2>&1 | head -1

# Node >= 20 is required to instantiate the songbook wasm: rustc 1.88 emits
# wasm typed function references, which node 18's V8 rejects. Debian
# bookworm ships node 18, so pin the LTS tarball (house style: pinned
# version + sha256, like tectonic above). Keep NODE_VERSION in step with
# supported LTS; the PDF smoke and worker run under it.
ARG NODE_VERSION=22.23.4
RUN curl -fsSL "https://nodejs.org/dist/v${NODE_VERSION}/node-v${NODE_VERSION}-linux-x64.tar.xz" -o /tmp/node.tar.xz \
    && echo "1805bcd35e6815559bd2eb97db438d3fdb0eb3a5e6a977b7c1badca76337e9ce  /tmp/node.tar.xz" | sha256sum -c - \
    && tar -xJf /tmp/node.tar.xz -C /usr/local --strip-components=1 --no-same-owner \
    && rm /tmp/node.tar.xz \
    && node --version && pdftotext -v 2>&1 | head -1
