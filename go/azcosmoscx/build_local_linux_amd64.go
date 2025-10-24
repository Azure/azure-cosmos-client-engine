// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

//go:build azcosmoscx_local && !dynamic && !musl && linux && amd64

package azcosmoscx

// #cgo debug LDFLAGS: ${SRCDIR}/../../artifacts/x86_64-unknown-linux-gnu/debug/lib/libcosmoscx.a -lgcc_s -lutil -lrt -lpthread -lm -ldl -lc
// #cgo !debug LDFLAGS: ${SRCDIR}/../../artifacts/x86_64-unknown-linux-gnu/release/lib/libcosmoscx.a -lgcc_s -lutil -lrt -lpthread -lm -ldl -lc
// #include <cosmoscx.h>
import "C"
