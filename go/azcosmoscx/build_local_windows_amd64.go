// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

//go:build azcosmoscx_local && !dynamic && windows && arm64

package azcosmoscx

// #cgo debug LDFLAGS: ${SRCDIR}/../../artifacts/x86_64-pc-windows-gnu/debug/lib/cosmoscx.a -lgdi32 -lkernel32 -lmsimg32 -lopengl32 -lwinspool -lkernel32 -lntdll -luserenv -lws2_32 -ldbghelp
// #cgo !debug LDFLAGS: ${SRCDIR}/../../artifacts/x86_64-pc-windows-gnu/release/lib/cosmoscx.a -lgdi32 -lkernel32 -lmsimg32 -lopengl32 -lwinspool -lkernel32 -lntdll -luserenv -lws2_32 -ldbghelp
// #include <cosmoscx.h>
import "C"
