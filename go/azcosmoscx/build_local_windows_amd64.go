// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

//go:build azcosmoscx_local && !dynamic && windows && arm64

package azcosmoscx

// #cgo debug LDFLAGS: ${SRCDIR}/../../artifacts/x86_64-pc-windows-msvc/debug/lib/cosmoscx.lib -lwinapi_gdi32 -lwinapi_kernel32 -lwinapi_msimg32 -lwinapi_opengl32 -lwinapi_winspool -lkernel32 -ladvapi32 -lntdll -luserenv -lws2_32 -ldbghelp
// #cgo !debug LDFLAGS: ${SRCDIR}/../../artifacts/x86_64-pc-windows-msvc/release/lib/cosmoscx.lib -lwinapi_gdi32 -lwinapi_kernel32 -lwinapi_msimg32 -lwinapi_opengl32 -lwinapi_winspool -lkernel32 -ladvapi32 -lntdll -luserenv -lws2_32 -ldbghelp
// #include <cosmoscx.h>
// #cgo noescape cosmoscx_v0_query_pipeline_create
// #cgo noescape cosmoscx_v0_query_pipeline_free
// #cgo noescape cosmoscx_v0_query_pipeline_query
// #cgo noescape cosmoscx_v0_query_pipeline_run
// #cgo noescape cosmoscx_v0_query_pipeline_provide_data
// #cgo noescape cosmoscx_v0_query_pipeline_free_result
// #cgo nocallback cosmoscx_v0_query_pipeline_create
// #cgo nocallback cosmoscx_v0_query_pipeline_free
// #cgo nocallback cosmoscx_v0_query_pipeline_query
// #cgo nocallback cosmoscx_v0_query_pipeline_run
// #cgo nocallback cosmoscx_v0_query_pipeline_provide_data
// #cgo nocallback cosmoscx_v0_query_pipeline_free_result
import "C"
