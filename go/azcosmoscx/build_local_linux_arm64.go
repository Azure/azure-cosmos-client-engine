// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

//go:build azcosmoscx_local && !dynamic && linux && arm64

package azcosmoscx

// #cgo debug LDFLAGS: ${SRCDIR}/../../artifacts/aarch64-unknown-linux-gnu/debug/lib/libcosmoscx.a -lgcc_s -lutil -lrt -lpthread -lm -ldl -lc
// #cgo !debug LDFLAGS: ${SRCDIR}/../../artifacts/aarch64-unknown-linux-gnu/release/lib/libcosmoscx.a -lgcc_s -lutil -lrt -lpthread -lm -ldl -lc
// #include <cosmoscx.h>
import "C"
