// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

//go:build azcosmoscx_local && !dynamic && windows && arm64

package azcosmoscx

// #cgo debug LDFLAGS: ${SRCDIR}/../../artifacts/x86_64-pc-windows-gnu/debug/lib/cosmoscx.a -lwinapi_gdi32 -lwinapi_kernel32 -lwinapi_msimg32 -lwinapi_opengl32 -lwinapi_winspool -lkernel32 -lntdll -luserenv -lws2_32 -ldbghelp
// #cgo !debug LDFLAGS: ${SRCDIR}/../../artifacts/x86_64-pc-windows-gnu/release/lib/cosmoscx.a -lwinapi_gdi32 -lwinapi_kernel32 -lwinapi_msimg32 -lwinapi_opengl32 -lwinapi_winspool -lkernel32 -lntdll -luserenv -lws2_32 -ldbghelp
// #include <cosmoscx.h>
import "C"
