package engine

// #cgo CFLAGS: -I${SRCDIR}/../../include
// #include <cosmoscx.h>
import "C"
import (
	"strings"
	"unsafe"
)

type Pipeline = *C.CosmosCxPipeline

func NewPipeline(queryPlan string, partitionKeyRanges string) (Pipeline, error) {
	queryPlanC := makeStr(queryPlan)
	pkRangesC := makeStr(partitionKeyRanges)

	r := C.cosmoscx_v0_query_pipeline_create(queryPlanC, pkRangesC)
	if err := mapErr(r.code); err != nil {
		return nil, err
	}

	return Pipeline(r.value), nil
}

// Free disposes of the native resources held by the pipeline.
// This should always be called when you're finished working with the pipeline.
func (p Pipeline) Free() {
	if p != nil {
		C.cosmoscx_v0_query_pipeline_free(p)
	}
}

func (p Pipeline) NextBatch() (PipelineResult, error) {
	r := C.cosmoscx_v0_query_pipeline_next_batch(p)
	if err := mapErr(r.code); err != nil {
		return nil, err
	}
	return PipelineResult(r.value), nil
}

func (p Pipeline) ProvideData(pkrangeid string, data string, continuation string) error {
	pkrangeidC := makeStr(pkrangeid)
	dataC := makeStr(data)
	continuationC := makeStr(continuation)
	return mapErr(C.cosmoscx_v0_query_pipeline_provide_data(p, pkrangeidC, dataC, continuationC))
}

type PipelineResult = *C.CosmosCxPipelineResult

func (r PipelineResult) Free() {
	if r != nil {
		C.cosmoscx_v0_query_pipeline_free_result(r)
	}
}

func (r PipelineResult) IsCompleted() bool {
	return bool(r.completed)
}

func (r PipelineResult) Items() []EngineString {
	ptr := (*EngineString)(r.items.data)
	return unsafe.Slice(ptr, r.items.len)
}

func (r PipelineResult) ItemsCloned() []string {
	items := r.Items()
	result := make([]string, 0, len(items))
	for _, item := range items {
		result = append(result, item.Clone())
	}
	return result
}

func (r PipelineResult) Requests() []DataRequest {
	ptr := (*DataRequest)(r.requests.data)
	return unsafe.Slice(ptr, r.requests.len)
}

type EngineString = C.CosmosCxOwnedString

// Borrow returns a "borrowed" copy of the string, as a Go String.
// The string returned here will become invalid when the PipelineResult that owned this is freed.
// Use Clone to create a copy of the string in Go memory
func (e EngineString) Borrow() string {
	return unsafe.String((*byte)(e.data), e.len)
}

// Clone creates a brand-new Go string, in Go-managed memory, containing the same data as the original string.
func (e EngineString) Clone() string {
	return strings.Clone(e.Borrow())
}

type DataRequest = C.CosmosCxDataRequest

func (r *DataRequest) PartitionKeyRangeID() EngineString {
	return EngineString(r.pkrangeid)
}

func (r *DataRequest) Continuation() EngineString {
	return EngineString(r.continuation)
}
