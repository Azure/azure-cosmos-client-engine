// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

//go:build !azcosmoscx_local && !dynamic && windows && amd64

package azcosmoscx

// #cgo LDFLAGS: ${SRCDIR}/libcosmoscx-vendor/x86_64-pc-windows-gnu/libcosmoscx.a -lgdi32 -lkernel32 -lmsimg32 -lopengl32 -lwinspool -lkernel32 -lntdll -luserenv -lws2_32 -ldbghelp
// #include <cosmoscx.h>
import "C"
