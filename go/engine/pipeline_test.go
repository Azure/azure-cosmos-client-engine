package engine_test

import (
	"testing"

	"github.com/Azure/azure-cosmos-client-engine/go/engine"
	"github.com/stretchr/testify/require"
)

func init() {
	// Always enable tracing
	engine.EnableTracing()
}

func TestAllocAndFree(t *testing.T) {
	plan := "{\"partitionedQueryExecutionInfoVersion\": 1, \"queryInfo\":{}, \"queryRanges\": []}"
	pkranges := []engine.PartitionKeyRange{
		engine.PartitionKeyRange{
			ID:           "1",
			MinInclusive: "00",
			MaxExclusive: "FF",
		},
	}
	pipeline, err := engine.NewPipeline(plan, pkranges)
	require.NoError(t, err)
	defer pipeline.Free()
}
