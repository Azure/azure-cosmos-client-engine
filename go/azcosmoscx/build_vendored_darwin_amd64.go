// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

//go:build !azcosmoscx_local && !dynamic && darwin && amd64

package azcosmoscx

// #cgo LDFLAGS: ${SRCDIR}/libcosmoscx-vendor/x86_64-apple-darwin/libcosmoscx.a -lSystem -lc -lm
// #include <cosmoscx.h>
import "C"
