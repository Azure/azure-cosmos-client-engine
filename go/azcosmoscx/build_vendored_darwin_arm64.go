// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

//go:build !azcosmoscx_local && !dynamic && darwin && arm64

package azcosmoscx

// #cgo LDFLAGS: ${SRCDIR}/libcosmoscx-vendor/aarch64-apple-darwin/libcosmoscx.a -lSystem -lc -lm
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
