// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

package azcosmoscx_test

import (
	"fmt"
	"testing"

	"github.com/Azure/azure-cosmos-client-engine/go/azcosmoscx"
	"github.com/Azure/azure-sdk-for-go/sdk/data/azcosmos/queryengine"
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
	pipeline, err := azcosmoscx.NewQueryEngine().CreateQueryPipeline("SELECT * FROM c", plan, pkranges)
	require.NoError(t, err)
	defer pipeline.Close()

	pipelineQuery := pipeline.Query()
	assert.Equal(t, "SELECT * FROM c", pipelineQuery)
}

func TestRewrittenQuery(t *testing.T) {
	plan := `{"partitionedQueryExecutionInfoVersion": 1, "queryInfo":{"rewrittenQuery": "WE REWRITTEN"}, "queryRanges": []}`
	pkranges := `{"PartitionKeyRanges":[{"id":"partition0","minInclusive":"00","maxExclusive":"FF"}]}`
	pipeline, err := azcosmoscx.NewQueryEngine().CreateQueryPipeline("SELECT * FROM c", plan, pkranges)
	require.NoError(t, err)
	defer pipeline.Close()

	pipelineQuery := pipeline.Query()
	assert.NoError(t, err)
	assert.Equal(t, "WE REWRITTEN", pipelineQuery)
}

func TestEmptyPipelineReturnsRequests(t *testing.T) {
	plan := `{"partitionedQueryExecutionInfoVersion": 1, "queryInfo":{}, "queryRanges": []}`
	pkranges := `{"PartitionKeyRanges":[{"id":"partition0","minInclusive":"00","maxExclusive":"99"},{"id":"partition1","minInclusive":"99","maxExclusive":"FF"}]}`
	pipeline, err := azcosmoscx.NewQueryEngine().CreateQueryPipeline("SELECT * FROM c", plan, pkranges)
	require.NoError(t, err)
	defer pipeline.Close()

	result, err := pipeline.Run()
	require.NoError(t, err)
	require.NotNil(t, result.Items)
	require.NotNil(t, result.Requests)

	assert.Empty(t, result.Items)
	assert.NotEmpty(t, result.Requests)

	for i, request := range result.Requests {
		expectedId := fmt.Sprintf("partition%d", i)
		assert.Equal(t, expectedId, request.PartitionKeyRangeID)
		assert.Empty(t, request.Continuation)
	}
}

func TestPipelineWithDataReturnsData(t *testing.T) {
	plan := "{\"partitionedQueryExecutionInfoVersion\": 1, \"queryInfo\":{}, \"queryRanges\": []}"
	pkranges := `{"PartitionKeyRanges":[{"id":"partition0","minInclusive":"00","maxExclusive":"99"},{"id":"partition1","minInclusive":"99","maxExclusive":"FF"}]}`
	pipeline, err := azcosmoscx.NewQueryEngine().CreateQueryPipeline("SELECT * FROM c", plan, pkranges)
	require.NoError(t, err)
	defer pipeline.Close()

	err = pipeline.ProvideData(queryengine.NewQueryResultString("partition0", `{
		"Documents": [1, 2]
	}`, "p0c1"))
	require.NoError(t, err)
	err = pipeline.ProvideData(queryengine.NewQueryResultString("partition1", `{
		"Documents": [3, 4]
	}`, "p1c1"))
	require.NoError(t, err)

	result, err := pipeline.Run()
	require.NoError(t, err)

	require.NotNil(t, result.Items)
	require.NotNil(t, result.Requests)

	assert.EqualValues(t, [][]byte{
		[]byte("1"),
		[]byte("2"),
	}, result.Items)

	assert.NotEmpty(t, result.Requests)

	assert.Equal(t, 2, len(result.Requests))
	assert.Equal(t, "partition0", result.Requests[0].PartitionKeyRangeID)
	assert.Equal(t, "p0c1", result.Requests[0].Continuation)
	assert.Equal(t, "partition1", result.Requests[1].PartitionKeyRangeID)
	assert.Equal(t, "p1c1", result.Requests[1].Continuation)

	// Provide empty data for the remaining partitions
	err = pipeline.ProvideData(queryengine.NewQueryResultString("partition0", `{"Documents":[]}`, ""))
	require.NoError(t, err)
	err = pipeline.ProvideData(queryengine.NewQueryResultString("partition1", `{"Documents":[]}`, ""))
	require.NoError(t, err)

	// And we should get the rest
	result, err = pipeline.Run()

	require.NoError(t, err)
	assert.EqualValues(t, [][]byte{
		[]byte("3"),
		[]byte("4"),
	}, result.Items)
}
