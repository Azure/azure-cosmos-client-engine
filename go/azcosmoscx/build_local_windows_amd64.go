// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

//go:build azcosmoscx_local && !dynamic && windows && arm64

package azcosmoscx

// #cgo debug LDFLAGS: ${SRCDIR}/../../artifacts/x86_64-pc-windows-msvc/debug/lib/cosmoscx.lib -lwinapi_gdi32 -lwinapi_kernel32 -lwinapi_msimg32 -lwinapi_opengl32 -lwinapi_winspool -lkernel32 -ladvapi32 -lntdll -luserenv -lws2_32 -ldbghelp
// #cgo !debug LDFLAGS: ${SRCDIR}/../../artifacts/x86_64-pc-windows-msvc/release/lib/cosmoscx.lib -lwinapi_gdi32 -lwinapi_kernel32 -lwinapi_msimg32 -lwinapi_opengl32 -lwinapi_winspool -lkernel32 -ladvapi32 -lntdll -luserenv -lws2_32 -ldbghelp
// #include <cosmoscx.h>
import "C"
