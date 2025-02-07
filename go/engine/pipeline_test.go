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
		{
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
		{
			ID:           "partition0",
			MinInclusive: "00",
			MaxExclusive: "99",
		},
		{
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
		assert.Equal(t, expectedId, request.PartitionKeyRangeID().Borrow())
		assert.Empty(t, request.Continuation().Borrow())
	}
}

func TestPipelineWithDataReturnsData(t *testing.T) {
	plan := "{\"partitionedQueryExecutionInfoVersion\": 1, \"queryInfo\":{}, \"queryRanges\": []}"
	pkranges := []engine.PartitionKeyRange{
		{
			ID:           "partition0",
			MinInclusive: "00",
			MaxExclusive: "99",
		},
		{
			ID:           "partition1",
			MinInclusive: "99",
			MaxExclusive: "FF",
		},
	}
	pipeline, err := engine.NewPipeline(plan, pkranges)
	require.NoError(t, err)
	defer pipeline.Free()

	err = pipeline.ProvideData("partition0", `[
		{"payload": 1},
		{"payload": 2}
	]`, "p0c1")
	require.NoError(t, err)
	err = pipeline.ProvideData("partition1", `[
		{"payload": 3},
		{"payload": 4}
	]`, "p1c1")
	require.NoError(t, err)

	result, err := pipeline.NextBatch()
	require.NoError(t, err)
	defer result.Free()

	assert.False(t, result.IsCompleted())

	items := result.ItemsCloned()
	assert.EqualValues(t, []string{
		"1",
		"2",
	}, items)

	requests := result.Requests()
	assert.NotEmpty(t, requests)

	for i, request := range requests {
		expectedId := fmt.Sprintf("partition%d", i)
		assert.Equal(t, expectedId, request.PartitionKeyRangeID().Borrow())

		expectedContinuation := fmt.Sprintf("p%dc1", i)
		assert.Equal(t, expectedContinuation, request.Continuation().Borrow())
	}

	// Provide empty data for the remaining partitions
	err = pipeline.ProvideData("partition0", `[]`, "")
	require.NoError(t, err)
	err = pipeline.ProvideData("partition1", `[]`, "")
	require.NoError(t, err)

	// And we should get the rest
	result, err = pipeline.NextBatch()
	require.NoError(t, err)
	defer result.Free()

	items = result.ItemsCloned()
	assert.EqualValues(t, []string{
		"3",
		"4",
	}, items)
}
