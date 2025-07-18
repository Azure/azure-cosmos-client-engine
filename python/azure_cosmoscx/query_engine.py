# Copyright (c) Microsoft Corporation.
# Licensed under the MIT License.

import azure.cosmos.query_engine

from . import _azure_cosmoscx


class QueryEngine(azure.cosmos.query_engine.QueryEngine):
    def create_pipeline(self, query, plan, pkranges):
        # We don't care about query arguments
        if isinstance(query, dict):
            query = query['query']

        if not isinstance(query, str):
            raise ValueError(
                "query must be a string or dictionary containing the 'query' key")

        return _azure_cosmoscx.QueryPipeline(query, plan, pkranges)
