# Copyright (c) Microsoft Corporation.
# Licensed under the MIT License.
import unittest

from run_integration import run_integration_test

class TestOrderBy(unittest.TestCase):
    def test_order_by(self):
        run_integration_test("../baselines/queries/order_by.json")
