// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

//go:build azcosmoscx_local && !dynamic && darwin && arm64

package azcosmoscx

// #cgo debug LDFLAGS: ${SRCDIR}/../../artifacts/aarch64-apple-darwin/debug/lib/libcosmoscx.a -lSystem -lc -lm
// #cgo !debug LDFLAGS: ${SRCDIR}/../../artifacts/aarch64-apple-darwin/release/lib/libcosmoscx.a -lSystem -lc -lm
// #include <cosmoscx.h>
import "C"
