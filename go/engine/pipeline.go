package engine

// #cgo CFLAGS: -I${SRCDIR}/../../include
// #include <cosmoscx.h>
import "C"

type Pipeline struct {
	ptr *C.CosmosCxPipeline
}

func NewPipeline(queryPlan string, partitionKeyRanges []PartitionKeyRange) (*Pipeline, error) {
	queryPlanC := makeStr(queryPlan)
	pkRanges := make([]C.CosmosCxPartitionKeyRange, 0, len(partitionKeyRanges))
	for _, srcRange := range partitionKeyRanges {
		id := makeStr(srcRange.ID)
		minInclusive := makeStr(srcRange.MinInclusive)
		maxExclusive := makeStr(srcRange.MaxExclusive)
		pkRanges = append(pkRanges, C.CosmosCxPartitionKeyRange{
			id:            id,
			min_inclusive: minInclusive,
			max_exclusive: maxExclusive,
		})
	}
	pkRangeList := makeSlice(pkRanges)

	ptr, err := unwrapResult(C.cosmoscx_v0_query_pipeline_create(queryPlanC, pkRangeList))
	if err != nil {
		return nil, err
	}
	return &Pipeline{ptr: (*C.CosmosCxPipeline)(ptr)}, nil
}

// Free disposes of the native resources held by the pipeline.
// This should always be called when you're finished working with the pipeline.
func (p *Pipeline) Free() {
	if p.ptr != nil {
		C.cosmoscx_v0_query_pipeline_free(p.ptr)
	}
}
