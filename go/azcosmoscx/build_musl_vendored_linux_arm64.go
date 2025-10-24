// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

//go:build !azcosmoscx_local && !dynamic && musl && linux && arm64

package azcosmoscx

// #cgo LDFLAGS: ${SRCDIR}/libcosmoscx-vendor/aarch64-unknown-linux-musl/libcosmoscx.a
// #include <cosmoscx.h>
import "C"
