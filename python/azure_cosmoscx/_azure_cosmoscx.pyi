from typing import Union

import azure.cosmos.query_engine


def version() -> str:
    pass


def enable_tracing() -> None:
    pass


class QueryEngine(azure.cosmos.query_engine.QueryEngine):
    pass


class QueryPipeline(azure.cosmos.query_engine.QueryPipeline):
    pass


class DataRequest(azure.cosmos.query_engine.DataRequest):
    pass


class PipelineResult(azure.cosmos.query_engine.PipelineResult):
    pass
