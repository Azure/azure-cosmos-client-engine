package engine_test

import (
	"fmt"
	"testing"

	"github.com/Azure/azure-cosmos-client-engine/go/engine"
	"github.com/stretchr/testify/assert"
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
			ID:           "partition0",
			MinInclusive: "00",
			MaxExclusive: "FF",
		},
	}
	pipeline, err := engine.NewPipeline(plan, pkranges)
	require.NoError(t, err)
	defer pipeline.Free()
}

func TestEmptyPipelineReturnsRequests(t *testing.T) {
	plan := "{\"partitionedQueryExecutionInfoVersion\": 1, \"queryInfo\":{}, \"queryRanges\": []}"
	pkranges := []engine.PartitionKeyRange{
		engine.PartitionKeyRange{
			ID:           "partition0",
			MinInclusive: "00",
			MaxExclusive: "99",
		},
		engine.PartitionKeyRange{
			ID:           "partition1",
			MinInclusive: "99",
			MaxExclusive: "FF",
		},
	}
	pipeline, err := engine.NewPipeline(plan, pkranges)
	require.NoError(t, err)
	defer pipeline.Free()

	result, err := pipeline.NextBatch()
	require.NoError(t, err)
	defer result.Free()

	assert.False(t, result.IsCompleted())

	items := result.Items()
	assert.Empty(t, items)

	requests := result.Requests()
	assert.NotEmpty(t, requests)

	for i, request := range requests {
		expectedId := fmt.Sprintf("partition%d", i)
		assert.Equal(t, expectedId, request.PartitionKeyRangeID())
		assert.Empty(t, request.Continuation())
	}
}
