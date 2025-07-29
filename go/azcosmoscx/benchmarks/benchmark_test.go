package benchmarks

import (
	"fmt"
	"testing"
	"time"

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

func createPartitionData(partitionID string, itemCount int) []BenchmarkItem {
	items := make([]BenchmarkItem, itemCount)
	for i := 0; i < itemCount; i++ {
		items[i] = NewBenchmarkItem(fmt.Sprintf("item_%d", i), partitionID, i)
	}
	return items
}

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

func fulfillDataRequests(pipeline queryengine.QueryPipeline, requests []queryengine.QueryRequest,
	partitionData map[string][]BenchmarkItem, ordered bool, latencyMs int) error {
	if len(requests) == 0 {
		return nil
	}

	if latencyMs > 0 {
		time.Sleep(time.Duration(latencyMs) * time.Millisecond)
	}

	var results []queryengine.QueryResult

	for _, request := range requests {
		partitionID := request.PartitionKeyRangeID
		items, exists := partitionData[partitionID]
		if !exists {
			continue
		}

		startIndex := 0
		if request.Continuation != "" {
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
			if ordered {
				documents[i] = fmt.Sprintf(`{"payload":{"id":"%s","partition_key":"%s","value":%d,"description":"%s"},"orderByItems":[{"item":%d}]}`,
					item.ID, item.PartitionKey, item.Value, item.Description, item.Value)
			} else {
				documents[i] = fmt.Sprintf(`{"id":"%s","partitionKey":"%s","value":%d,"description":"%s"}`,
					item.ID, item.PartitionKey, item.Value, item.Description)
			}
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

	for _, result := range results {
		err := pipeline.ProvideData(result)
		if err != nil {
			return err
		}
	}

	return nil
}

func runBenchmarkScenario(b *testing.B, partitionData map[string][]BenchmarkItem, ordered bool, latencyMs int) (int, error) {
	queryPlan := `{"partitionedQueryExecutionInfoVersion": 1, "queryInfo":{}, "queryRanges": []}`
	partitionRanges := createPartitionKeyRanges(PartitionCount)

	pipeline, err := azcosmoscx.NewQueryEngine().CreateQueryPipeline("SELECT * FROM c", queryPlan, partitionRanges)
	if err != nil {
		return 0, err
	}
	defer pipeline.Close()

	totalItems := 0

	for !pipeline.IsComplete() {
		result, err := pipeline.Run()
		if err != nil {
			return 0, err
		}

		totalItems += len(result.Items)

		if len(result.Requests) > 0 {
			err = fulfillDataRequests(pipeline, result.Requests, partitionData, ordered, latencyMs)
			if err != nil {
				return 0, err
			}
		}
	}

	return totalItems, nil
}

func BenchmarkPipelineThroughput_Unordered_0ms(b *testing.B) {
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
		iterItems, err := runBenchmarkScenario(b, partitionData, false, 0) // 0ms latency
		if err != nil {
			b.Fatal(err)
		}
		totalItems += iterItems
	}

	b.ReportMetric(float64(totalItems)/float64(b.Elapsed().Seconds()), "items/s")
}

func BenchmarkPipelineThroughput_Unordered_5ms(b *testing.B) {
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
		iterItems, err := runBenchmarkScenario(b, partitionData, false, 5) // 5ms latency
		if err != nil {
			b.Fatal(err)
		}
		totalItems += iterItems
	}

	b.ReportMetric(float64(totalItems)/float64(b.Elapsed().Seconds()), "items/s")
}

func BenchmarkPipelineThroughput_Unordered_10ms(b *testing.B) {
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
		iterItems, err := runBenchmarkScenario(b, partitionData, false, 10) // 10ms latency
		if err != nil {
			b.Fatal(err)
		}
		totalItems += iterItems
	}

	b.ReportMetric(float64(totalItems)/float64(b.Elapsed().Seconds()), "items/s")
}

func BenchmarkPipelineThroughput_Ordered_0ms(b *testing.B) {
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
		iterItems, err := runBenchmarkScenario(b, partitionData, true, 0) // 0ms latency
		if err != nil {
			b.Fatal(err)
		}
		totalItems += iterItems
	}

	b.ReportMetric(float64(totalItems)/float64(b.Elapsed().Seconds()), "items/s")
}

func BenchmarkPipelineThroughput_Ordered_5ms(b *testing.B) {
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
		iterItems, err := runBenchmarkScenario(b, partitionData, true, 5) // 5ms latency
		if err != nil {
			b.Fatal(err)
		}
		totalItems += iterItems
	}

	b.ReportMetric(float64(totalItems)/float64(b.Elapsed().Seconds()), "items/s")
}

func BenchmarkPipelineThroughput_Ordered_10ms(b *testing.B) {
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
		iterItems, err := runBenchmarkScenario(b, partitionData, true, 10) // 10ms latency
		if err != nil {
			b.Fatal(err)
		}
		totalItems += iterItems
	}

	b.ReportMetric(float64(totalItems)/float64(b.Elapsed().Seconds()), "items/s")
}
