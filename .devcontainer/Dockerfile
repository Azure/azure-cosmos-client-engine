# Copyright (c) Microsoft Corporation.
# Licensed under the MIT License.

FROM mcr.microsoft.com/devcontainers/rust:1-1-bullseye

# Install APT packages:
# * aarch64-linux-gnu-gcc: Cross compiler for ARM64 Linux targets.
RUN apt-get update && export DEBIAN_FRONTEND=noninteractive \
    && apt-get -y install --no-install-recommends \
    aarch64-linux-gnu-gcc \
    build-essential

USER vscode

# Install nightly toolchain to give access to nightly features for docs-site build.
RUN rustup toolchain install nightly

# Install cargo dependencies
RUN cargo install --locked \
    cbindgen \
    cargo-watch \
    http-server \
    just \
    maturin