// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

//go:build azcosmoscx_local && !dynamic && windows && arm64

package azcosmoscx

// #cgo debug LDFLAGS: ${SRCDIR}/../../artifacts/x86_64-pc-windows-gnu/debug/lib/cosmoscx.a
// #cgo !debug LDFLAGS: ${SRCDIR}/../../artifacts/x86_64-pc-windows-gnu/release/lib/cosmoscx.a
// #include <cosmoscx.h>
import "C"
