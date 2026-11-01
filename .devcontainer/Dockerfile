FROM mcr.microsoft.com/devcontainers/rust:1-1-bookworm

ARG TECTONIC_VERSION=0.15.0
RUN curl -Ls "https://github.com/tectonic-typesetting/tectonic/releases/download/tectonic%40${TECTONIC_VERSION}/tectonic-${TECTONIC_VERSION}-x86_64-unknown-linux-musl.tar.gz" \
    | tar xz -C /usr/local/bin tectonic \
    && tectonic --version
