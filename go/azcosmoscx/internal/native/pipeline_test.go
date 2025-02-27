package native_test

import (
	"fmt"
	"testing"

	"github.com/Azure/azure-cosmos-client-engine/go/azcosmoscx"
	"github.com/Azure/azure-cosmos-client-engine/go/azcosmoscx/internal/native"
	"github.com/stretchr/testify/assert"
	"github.com/stretchr/testify/require"
)

func init() {
	// Always enable tracing
	azcosmoscx.EnableTracing()
}

func TestAllocAndFree(t *testing.T) {
	plan := `{"partitionedQueryExecutionInfoVersion": 1, "queryInfo":{}, "queryRanges": []}`
	pkranges := `{"PartitionKeyRanges":[{"id":"partition0","minInclusive":"00","maxExclusive":"FF"}]}`
	pipeline, err := native.NewPipeline("SELECT * FROM c", plan, pkranges)
	require.NoError(t, err)
	defer pipeline.Free()

	pipelineQuery, err := pipeline.Query()
	assert.NoError(t, err)
	assert.Equal(t, "SELECT * FROM c", pipelineQuery)
}

func TestRewrittenQuery(t *testing.T) {
	plan := `{"partitionedQueryExecutionInfoVersion": 1, "queryInfo":{"rewrittenQuery": "WE REWRITTEN"}, "queryRanges": []}`
	pkranges := `{"PartitionKeyRanges":[{"id":"partition0","minInclusive":"00","maxExclusive":"FF"}]}`
	pipeline, err := native.NewPipeline("SELECT * FROM c", plan, pkranges)
	require.NoError(t, err)
	defer pipeline.Free()

	pipelineQuery, err := pipeline.Query()
	assert.NoError(t, err)
	assert.Equal(t, "WE REWRITTEN", pipelineQuery)
}

func TestEmptyPipelineReturnsRequests(t *testing.T) {
	plan := `{"partitionedQueryExecutionInfoVersion": 1, "queryInfo":{}, "queryRanges": []}`
	pkranges := `{"PartitionKeyRanges":[{"id":"partition0","minInclusive":"00","maxExclusive":"99"},{"id":"partition1","minInclusive":"99","maxExclusive":"FF"}]}`
	pipeline, err := native.NewPipeline("SELECT * FROM c", plan, pkranges)
	require.NoError(t, err)
	defer pipeline.Free()

	result, err := pipeline.NextBatch()
	require.NoError(t, err)
	defer result.Free()

	assert.False(t, result.IsCompleted())

	items, err := result.Items()
	require.NoError(t, err)
	assert.Empty(t, items)

	requests, err := result.Requests()
	require.NoError(t, err)
	assert.NotEmpty(t, requests)

	for i, request := range requests {
		expectedId := fmt.Sprintf("partition%d", i)
		assert.Equal(t, expectedId, request.PartitionKeyRangeID().BorrowString())
		assert.Empty(t, request.Continuation().BorrowString())
	}
}

func TestPipelineWithDataReturnsData(t *testing.T) {
	plan := "{\"partitionedQueryExecutionInfoVersion\": 1, \"queryInfo\":{}, \"queryRanges\": []}"
	pkranges := `{"PartitionKeyRanges":[{"id":"partition0","minInclusive":"00","maxExclusive":"99"},{"id":"partition1","minInclusive":"99","maxExclusive":"FF"}]}`
	pipeline, err := native.NewPipeline("SELECT * FROM c", plan, pkranges)
	require.NoError(t, err)
	defer pipeline.Free()

	err = pipeline.ProvideData("partition0", `{
		"Documents": [1, 2]
	}`, "p0c1")
	require.NoError(t, err)
	err = pipeline.ProvideData("partition1", `{
		"Documents": [3, 4]
	}`, "p1c1")
	require.NoError(t, err)

	result, err := pipeline.NextBatch()
	require.NoError(t, err)
	defer result.Free()

	assert.False(t, result.IsCompleted())

	items, err := result.ItemsCloned()
	require.NoError(t, err)
	assert.EqualValues(t, [][]byte{
		[]byte("1"),
		[]byte("2"),
	}, items)

	requests, err := result.Requests()
	require.NoError(t, err)
	assert.NotEmpty(t, requests)

	assert.Equal(t, 2, len(requests))
	assert.Equal(t, "partition0", requests[0].PartitionKeyRangeID().BorrowString())
	assert.Equal(t, "p0c1", requests[0].Continuation().BorrowString())
	assert.Equal(t, "partition1", requests[1].PartitionKeyRangeID().BorrowString())
	assert.Equal(t, "p1c1", requests[1].Continuation().BorrowString())

	// Provide empty data for the remaining partitions
	err = pipeline.ProvideData("partition0", `{"Documents":[]}`, "")
	require.NoError(t, err)
	err = pipeline.ProvideData("partition1", `{"Documents":[]}`, "")
	require.NoError(t, err)

	// And we should get the rest
	result, err = pipeline.NextBatch()
	require.NoError(t, err)
	defer result.Free()

	items, err = result.ItemsCloned()
	require.NoError(t, err)
	assert.EqualValues(t, [][]byte{
		[]byte("3"),
		[]byte("4"),
	}, items)
}
