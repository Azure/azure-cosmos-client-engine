// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

package azcosmoscx

// We can mark every function as both noescape and nocallback, because we never retain a Go pointer beyond the C function call and we never call back in to Go code.

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
import (
	"bytes"
	"strings"
	"unsafe"
)

type Pipeline struct {
	ptr *C.CosmosCxPipeline
}

func newPipeline(query string, queryPlan string, partitionKeyRanges string) (*Pipeline, error) {
	queryC := makeStr(query)
	queryPlanC := makeStr(queryPlan)
	pkRangesC := makeStr(partitionKeyRanges)

	r := C.cosmoscx_v0_query_pipeline_create(queryC, queryPlanC, pkRangesC)
	if err := mapErr(r.code); err != nil {
		return nil, err
	}

	return &Pipeline{r.value}, nil
}

// IsFreed returns a boolean indicating whether the pipeline has been freed.
func (p *Pipeline) IsFreed() bool {
	return p.ptr == nil
}

// Free disposes of the native resources held by the pipeline.
// This should always be called when you're finished working with the pipeline.
func (p *Pipeline) Free() {
	if p.ptr != nil {
		C.cosmoscx_v0_query_pipeline_free(p.ptr)
		p.ptr = nil
	}
}

// Query gets the, possibly rewritten, query that should be used when issuing queries to satisfy DataRequests.
func (p *Pipeline) Query() (string, error) {
	r := C.cosmoscx_v0_query_pipeline_query(p.ptr)
	if err := mapErr(r.code); err != nil {
		return "", err
	}
	s := unsafe.String((*byte)(r.value.data), r.value.len)

	// Clone the string into Go memory
	return strings.Clone(s), nil
}

func (p *Pipeline) NextBatch() (*PipelineResult, error) {
	r := C.cosmoscx_v0_query_pipeline_run(p.ptr)
	if err := mapErr(r.code); err != nil {
		return nil, err
	}

	return &PipelineResult{r.value}, nil
}

func (p *Pipeline) ProvideData(pkrangeid string, data string, continuation string) error {
	pkrangeidC := makeStr(pkrangeid)
	dataC := makeStr(data)
	continuationC := makeStr(continuation)
	return mapErr(C.cosmoscx_v0_query_pipeline_provide_data(p.ptr, pkrangeidC, dataC, continuationC))
}

type PipelineResult struct {
	ptr *C.CosmosCxPipelineResult
}

func (r *PipelineResult) Free() {
	if r.ptr != nil {
		C.cosmoscx_v0_query_pipeline_free_result(r.ptr)
		r.ptr = nil
	}
}

func (r *PipelineResult) IsCompleted() bool {
	if r.ptr == nil {
		return true
	}
	return bool(r.ptr.completed)
}

func (r *PipelineResult) Items() ([]EngineString, error) {
	if r.ptr == nil {
		return nil, &Error{C.COSMOS_CX_RESULT_CODE_ARGUMENT_NULL}
	}
	ptr := (*EngineString)(r.ptr.items.data)
	return unsafe.Slice(ptr, r.ptr.items.len), nil
}

func (r *PipelineResult) ItemsCloned() ([][]byte, error) {
	items, err := r.Items()
	if err != nil {
		return nil, err
	}

	result := make([][]byte, 0, len(items))
	for _, item := range items {
		result = append(result, item.CloneBytes())
	}
	return result, nil
}

func (r *PipelineResult) Requests() ([]DataRequest, error) {
	if r.ptr == nil {
		return nil, &Error{C.COSMOS_CX_RESULT_CODE_ARGUMENT_NULL}
	}
	ptr := (*DataRequest)(r.ptr.requests.data)
	return unsafe.Slice(ptr, r.ptr.requests.len), nil
}

type EngineString C.CosmosCxOwnedString

// BorrowString returns a "borrowed" copy of the string, as a Go String.
// The string returned here will become invalid when the PipelineResult that owned this is freed.
// Use Clone to create a copy of the string in Go memory
func (e EngineString) BorrowString() string {
	return unsafe.String((*byte)(e.data), e.len)
}

// BorrowBytes returns a "borrowed" copy of the string as a Go slice of bytes.
// The string returned here will become invalid when the PipelineResult that owned this is freed.
// Use Clone to create a copy of the string in Go memory
func (e EngineString) BorrowBytes() []byte {
	return unsafe.Slice((*byte)(e.data), e.len)
}

// CloneString creates a brand-new Go string, in Go-managed memory, containing the same data as the original string.
func (e EngineString) CloneString() string {
	return strings.Clone(e.BorrowString())
}

// CloneBytes creates a brand-new slice of bytes, in Go-managed memory, containing the same data as the original string.
func (e EngineString) CloneBytes() []byte {
	return bytes.Clone(e.BorrowBytes())
}

type DataRequest C.CosmosCxDataRequest

func (r *DataRequest) PartitionKeyRangeID() EngineString {
	return EngineString(r.pkrangeid)
}

func (r *DataRequest) Continuation() EngineString {
	return EngineString(r.continuation)
}
