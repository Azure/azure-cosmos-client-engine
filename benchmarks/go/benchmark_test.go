package benchmarks

import (
	"fmt"
	"testing"

	"github.com/Azure/azure-cosmos-client-engine/go/azcosmoscx"
	"github.com/Azure/azure-sdk-for-go/sdk/data/azcosmos/queryengine"
)

// Configuration constants
const (
	PartitionCount = 4
	PageSize       = 100
)

// BenchmarkItem represents a simple test item for benchmarking
type BenchmarkItem struct {
	ID           string `json:"id"`
	PartitionKey string `json:"partitionKey"`
	Value        int    `json:"value"`
	Description  string `json:"description"`
}

// NewBenchmarkItem creates a new benchmark item
func NewBenchmarkItem(id, partitionKey string, value int) BenchmarkItem {
	return BenchmarkItem{
		ID:           id,
		PartitionKey: partitionKey,
		Value:        value,
		Description:  fmt.Sprintf("Item %s in partition %s", id, partitionKey),
	}
}

// createPartitionData generates test data for a partition
func createPartitionData(partitionID string, itemCount int) []BenchmarkItem {
	items := make([]BenchmarkItem, itemCount)
	for i := 0; i < itemCount; i++ {
		items[i] = NewBenchmarkItem(fmt.Sprintf("item_%d", i), partitionID, i)
	}
	return items
}

// createPartitionKeyRanges creates partition key ranges for testing
func createPartitionKeyRanges(count int) string {
	ranges := make([]string, count)
	for i := 0; i < count; i++ {
		ranges[i] = fmt.Sprintf(`{"id":"partition_%d","minInclusive":"%02X","maxExclusive":"%02X"}`,
			i, i*10, (i+1)*10)
	}

	rangesList := ""
	for i, r := range ranges {
		if i > 0 {
			rangesList += ","
		}
		rangesList += r
	}

	return fmt.Sprintf(`{"PartitionKeyRanges":[%s]}`, rangesList)
}

// fulfillDataRequests handles data requests from the pipeline using batch API
func fulfillDataRequests(pipeline queryengine.QueryPipeline, requests []queryengine.QueryRequest,
	partitionData map[string][]BenchmarkItem) error {
	if len(requests) == 0 {
		return nil
	}

	// Collect all the results to provide in batch
	var results []queryengine.QueryResult

	for _, request := range requests {
		partitionID := request.PartitionKeyRangeID
		items, exists := partitionData[partitionID]
		if !exists {
			continue
		}

		// Calculate which items to return based on continuation
		startIndex := 0
		if request.Continuation != "" {
			// Parse continuation as integer index
			fmt.Sscanf(request.Continuation, "%d", &startIndex)
		}

		endIndex := startIndex + PageSize
		if endIndex > len(items) {
			endIndex = len(items)
		}

		// Create response data
		responseItems := items[startIndex:endIndex]
		documents := make([]string, len(responseItems))
		for i, item := range responseItems {
			documents[i] = fmt.Sprintf(`{"id":"%s","partitionKey":"%s","value":%d,"description":"%s"}`,
				item.ID, item.PartitionKey, item.Value, item.Description)
		}

		// Determine continuation token
		var continuation string
		if endIndex < len(items) {
			continuation = fmt.Sprintf("%d", endIndex)
		}

		// Format response data
		documentsStr := ""
		for i, doc := range documents {
			if i > 0 {
				documentsStr += ","
			}
			documentsStr += doc
		}
		responseData := fmt.Sprintf(`{"Documents":[%s]}`, documentsStr)

		// Add to batch
		results = append(results, queryengine.NewQueryResultString(partitionID, responseData, continuation))
	}

	// Use batch API if available, otherwise fall back to individual calls
	if batchPipeline, ok := pipeline.(interface {
		ProvideDataBatch([]queryengine.QueryResult) error
	}); ok {
		return batchPipeline.ProvideDataBatch(results)
	} else {
		// Fallback to individual calls
		for _, result := range results {
			err := pipeline.ProvideData(result)
			if err != nil {
				return err
			}
		}
		return nil
	}
}

// runBenchmarkScenario executes a single benchmark scenario
func runBenchmarkScenario(b *testing.B, partitionData map[string][]BenchmarkItem) (int, error) {
	// Create query plan and partition ranges
	queryPlan := `{"partitionedQueryExecutionInfoVersion": 1, "queryInfo":{}, "queryRanges": []}`
	partitionRanges := createPartitionKeyRanges(PartitionCount)

	// Create pipeline
	pipeline, err := azcosmoscx.NewQueryEngine().CreateQueryPipeline("SELECT * FROM c", queryPlan, partitionRanges)
	if err != nil {
		return 0, err
	}
	defer pipeline.Close()

	totalItems := 0

	// Run the pipeline until completion
	for !pipeline.IsComplete() {
		result, err := pipeline.Run()
		if err != nil {
			return 0, err
		}

		// Count items yielded by this turn
		totalItems += len(result.Items)

		// If there are data requests, fulfill them
		if len(result.Requests) > 0 {
			err = fulfillDataRequests(pipeline, result.Requests, partitionData)
			if err != nil {
				return 0, err
			}
		}
	}

	return totalItems, nil
}

// BenchmarkPipelineThroughput_Unordered_100 benchmarks unordered pipeline with 100 items per partition
func BenchmarkPipelineThroughput_Unordered_100(b *testing.B) {
	b.ResetTimer()

	itemsPerPartition := 100
	b.SetBytes(int64(PartitionCount * itemsPerPartition))

	// Pre-create test data
	partitionData := make(map[string][]BenchmarkItem)
	for i := 0; i < PartitionCount; i++ {
		partitionID := fmt.Sprintf("partition_%d", i)
		partitionData[partitionID] = createPartitionData(partitionID, itemsPerPartition)
	}

	totalItems := 0
	for b.Loop() {
		iterItems, err := runBenchmarkScenario(b, partitionData)
		if err != nil {
			b.Fatal(err)
		}
		totalItems += iterItems
	}

	b.ReportMetric(float64(totalItems)/float64(b.Elapsed().Seconds()), "items/s")
}
