import unittest
import azure_cosmoscx


class TestPipeline(unittest.TestCase):
    def test_native_interop(self):
        plan = {
            "partitionedQueryExecutionInfoVersion": 1,
            "queryInfo": {
                "distinctType": "None",
            },
            "queryRanges": []
        }
        pkranges = [
            {
                "id": "partition0",
                "minInclusive": "00",
                "maxExclusive": "FF"
            }
        ]
        pipeline = azure_cosmoscx.QueryEngine().create_pipeline(
            "SELECT * FROM c", plan, pkranges)

        self.assertEqual("SELECT * FROM c", pipeline.query())

    def test_query_rewriting(self):
        plan = {
            "partitionedQueryExecutionInfoVersion": 1,
            "queryInfo": {
                "distinctType": "None",
                "rewrittenQuery": "WAS REWRITTEN",
            },
            "queryRanges": []
        }
        pkranges = [
            {
                "id": "partition0",
                "minInclusive": "00",
                "maxExclusive": "FF"
            }
        ]
        pipeline = azure_cosmoscx.QueryEngine().create_pipeline(
            "SELECT * FROM c", plan, pkranges)

        self.assertEqual("WAS REWRITTEN", pipeline.query())

    def test_empty_pipeline_returns_requests(self):
        plan = {
            "partitionedQueryExecutionInfoVersion": 1,
            "queryInfo": {
                "distinctType": "None",
            },
            "queryRanges": []
        }
        pkranges = [
            {
                "id": "partition0",
                "minInclusive": "00",
                "maxExclusive": "99"
            },
            {
                "id": "partition1",
                "minInclusive": "99",
                "maxExclusive": "FF"
            }
        ]
        pipeline = azure_cosmoscx.QueryEngine().create_pipeline(
            "SELECT * FROM c", plan, pkranges)

        result = pipeline.run()
        self.assertFalse(result.terminated)

        self.assertEqual(0, len(result.items))

        requests = [(r.pkrange_id, r.continuation)
                    for r in result.requests]
        self.assertEqual([
            ("partition0", None),
            ("partition1", None)
        ], requests)

    def test_pipeline_with_data_returns_data(self):
        plan = {
            "partitionedQueryExecutionInfoVersion": 1,
            "queryInfo": {
                "distinctType": "None",
            },
            "queryRanges": []
        }
        pkranges = [
            {
                "id": "partition0",
                "minInclusive": "00",
                "maxExclusive": "99"
            },
            {
                "id": "partition1",
                "minInclusive": "99",
                "maxExclusive": "FF"
            }
        ]
        pipeline = azure_cosmoscx.QueryEngine().create_pipeline(
            "SELECT * FROM c", plan, pkranges)

        pipeline.provide_data(
            "partition0", [1, 2], "p0c0")
        pipeline.provide_data(
            "partition1", [3, 4], "p1c0")

        result = pipeline.run()
        self.assertFalse(result.terminated)

        self.assertEqual([1, 2], result.items)

        requests = [(r.pkrange_id, r.continuation)
                    for r in result.requests]
        self.assertEqual([
            ("partition0", "p0c0"),
            ("partition1", "p1c0"),
        ], requests)

        pipeline.provide_data(
            "partition0", [], None)
        pipeline.provide_data(
            "partition1", [], None)

        result = pipeline.run()
        self.assertTrue(result.terminated)
        self.assertEqual([3, 4], result.items)
        self.assertEqual([], result.requests)

    def test_pipeline_with_order_by(self):
        plan = {
            "partitionedQueryExecutionInfoVersion": 1,
            "queryInfo": {
                "distinctType": "None",
                "orderBy": ["Ascending"],
            },
            "queryRanges": []
        }
        pkranges = [
            {
                "id": "partition0",
                "minInclusive": "00",
                "maxExclusive": "99"
            },
            {
                "id": "partition1",
                "minInclusive": "99",
                "maxExclusive": "FF"
            }
        ]
        pipeline = azure_cosmoscx.QueryEngine().create_pipeline(
            "SELECT * FROM c", plan, pkranges)

        pipeline.provide_data(
            "partition0", [
                {"orderByItems": [{"item": 4}], "payload": 1},
                {"orderByItems": [{"item": 2}], "payload": 2}
            ], "p0c0")
        pipeline.provide_data(
            "partition1", [
                {"orderByItems": [{"item": 1}], "payload": 3},
                {"orderByItems": [{"item": 3}], "payload": 4}
            ], "p1c0")

        result = pipeline.run()
        self.assertFalse(result.terminated)

        self.assertEqual([3, 4], result.items)

        requests = [(r.pkrange_id, r.continuation)
                    for r in result.requests]
        self.assertEqual([
            ("partition0", "p0c0"),
            ("partition1", "p1c0"),
        ], requests)

        pipeline.provide_data(
            "partition1", [], None)

        result = pipeline.run()
        self.assertFalse(result.terminated)

        self.assertEqual([1, 2], result.items)
        requests = [(r.pkrange_id, r.continuation)
                    for r in result.requests]
        self.assertEqual([("partition0", "p0c0")], requests)

        pipeline.provide_data(
            "partition0", [], None)

        result = pipeline.run()
        self.assertTrue(result.terminated)
        self.assertEqual([], result.items)
        self.assertEqual([], result.requests)
