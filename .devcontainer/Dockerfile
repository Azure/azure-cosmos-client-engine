# Copyright (c) Microsoft Corporation.
# Licensed under the MIT License.

FROM mcr.microsoft.com/devcontainers/rust:1-1-bullseye

USER vscode

# Install nightly toolchain to give access to nightly features for docs-site build.
RUN rustup toolchain install nightly

# Install cargo dependencies
RUN cargo install --locked \
    cbindgen \
    cargo-watch \
    http-server