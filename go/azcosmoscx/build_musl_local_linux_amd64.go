// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

//go:build azcosmoscx_local && !dynamic && musl && linux && amd64

package azcosmoscx

// #cgo debug LDFLAGS: ${SRCDIR}/../../artifacts/x86_64-unknown-linux-musl/debug/lib/libcosmoscx.a
// #cgo !debug LDFLAGS: ${SRCDIR}/../../artifacts/x86_64-unknown-linux-musl/release/lib/libcosmoscx.a
// #include <cosmoscx.h>
import "C"
