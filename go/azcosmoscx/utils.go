// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

package azcosmoscx

import "unsafe"

// #cgo CFLAGS: -I${SRCDIR}/include
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

func makeStr(s string) C.CosmosCxStr {
	ptr := unsafe.StringData(s)
	return C.CosmosCxStr{
		data: (*C.uint8_t)(ptr),
		len:  C.uintptr_t(len(s)),
	}
}
