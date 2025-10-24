// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

//go:build azcosmoscx_local && !dynamic && musl && linux && arm64

package azcosmoscx

// #cgo debug LDFLAGS: ${SRCDIR}/../../artifacts/aarch64-unknown-linux-musl/debug/lib/libcosmoscx.a
// #cgo !debug LDFLAGS: ${SRCDIR}/../../artifacts/aarch64-unknown-linux-musl/release/lib/libcosmoscx.a
// #include <cosmoscx.h>
import "C"
