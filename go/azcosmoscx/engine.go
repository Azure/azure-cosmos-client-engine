// Copyright (c) Microsoft Corporation.
// Licensed under the MIT License.

package azcosmoscx

// TODO: We need to evaluate how to distribute the native library itself and how best to link it (static/shared).

// #cgo CFLAGS: -I${SRCDIR}/include
// #include <cosmoscx.h>
import "C"

import (
	"github.com/Azure/azure-sdk-for-go/sdk/data/azcosmos/queryengine"
)

func Version() string {
	return C.GoString(C.cosmoscx_version())
}

// EnableTracing enables Cosmos Client Engine tracing.
// Once enabled, tracing cannot be disabled (for now). Tracing is controlled by setting the COSMOSCX_LOG environment variable, using the syntax of the `RUST_LOG` (https://docs.rs/env_logger/latest/env_logger/#enabling-logging) env var.
func EnableTracing() {
	C.cosmoscx_v0_tracing_enable()
}

type nativeQueryEngine struct {
}

// NewQueryEngine creates a new azcosmoscx query engine.
func NewQueryEngine() queryengine.QueryEngine {
	return &nativeQueryEngine{}
}

// CreateQueryPipeline creates a new query pipeline from the provided plan and partition key ranges.
func (e *nativeQueryEngine) CreateQueryPipeline(query string, plan string, pkranges string) (queryengine.QueryPipeline, error) {
	pipeline, err := newPipeline(query, plan, pkranges)
	if err != nil {
		return nil, err
	}

	query, err = pipeline.Query()
	if err != nil {
		// The only expected error here is if the pipeline is null. Still, we should report it.
		pipeline.Free()
		return nil, err
	}
	return &clientEngineQueryPipeline{pipeline, query, false}, nil
}

func (e *nativeQueryEngine) SupportedFeatures() string {
	return C.GoString(C.cosmoscx_v0_query_supported_features())
}

type clientEngineQueryPipeline struct {
	pipeline  *Pipeline
	query     string
	completed bool
}

// GetRewrittenQuery returns the query text, possibly rewritten by the gateway, which will be used for per-partition queries.
func (p *clientEngineQueryPipeline) Query() string {
	return p.query
}

func (p *clientEngineQueryPipeline) Close() {
	p.pipeline.Free()
}

// IsComplete gets a boolean indicating if the pipeline has concluded
func (p *clientEngineQueryPipeline) IsComplete() bool {
	return p.pipeline.IsFreed() || p.completed
}

// NextBatch gets the next batch of items, which will be empty if there are no more items in the buffer.
// The number of items retrieved will be capped by the provided maxPageSize if it is positive.
// Any remaining items will be returned by the next call to NextBatch.
func (p *clientEngineQueryPipeline) Run() (*queryengine.PipelineResult, error) {
	result, err := p.pipeline.NextBatch()
	defer result.Free()
	if err != nil {
		return nil, err
	}

	p.completed = result.IsCompleted()

	items, err := result.ItemsCloned()
	if err != nil {
		return nil, err
	}

	sourceRequests, err := result.Requests()
	if err != nil {
		return nil, err
	}
	requests := make([]queryengine.QueryRequest, 0, len(sourceRequests))
	for _, request := range sourceRequests {
		requests = append(requests, queryengine.QueryRequest{
			PartitionKeyRangeID: string(request.PartitionKeyRangeID().CloneString()),
			Continuation:        string(request.Continuation().CloneString()),
			Query:               string(request.Query().CloneString()),
		})
	}
	return &queryengine.PipelineResult{
		IsCompleted: p.completed,
		Items:       items,
		Requests:    requests,
	}, nil
}

// ProvideData provides more data for a given partition key range ID, using data retrieved from the server in response to making a DataRequest.
func (p *clientEngineQueryPipeline) ProvideData(results []queryengine.QueryResult) error {
	return p.pipeline.ProvideData(results)
}

// CreateReadManyPipeline creates the relevant partition-scoped queries for executing the read many operation along with the pipeline to run them.
func (e *nativeQueryEngine) CreateReadManyPipeline(items string, pkranges string, pkKind string, pkVersion int32) (queryengine.QueryPipeline, error) {
	pipeline, err := newReadManyPipeline(items, pkranges, pkKind, pkVersion)
	if err != nil {
		return nil, err
	}

	return &clientEngineQueryPipeline{pipeline, "", false}, nil
}
