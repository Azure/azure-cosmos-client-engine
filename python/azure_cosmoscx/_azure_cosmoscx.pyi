from typing import Union

import azure.cosmos.query_engine


def version() -> str:
    pass


class NativeQueryEngine(azure.cosmos.query_engine.QueryEngine):
    pass


class NativeQueryPipeline(azure.cosmos.query_engine.QueryPipeline):
    pass


class DataRequest(azure.cosmos.query_engine.DataRequest):
    pass


class PipelineResult(azure.cosmos.query_engine.PipelineResult):
    pass
