// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

//go:build !azcosmoscx_local && !dynamic && linux && amd64

package azcosmoscx

// #cgo LDFLAGS: ${SRCDIR}/libcosmoscx-vendor/x86_64-unknown-linux-gnu/libcosmoscx.a -lgcc_s -lutil -lrt -lpthread -lm -ldl -lc
// #include <cosmoscx.h>
import "C"
