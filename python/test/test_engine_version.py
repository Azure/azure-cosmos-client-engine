# Copyright (c) Microsoft Corporation.
# Licensed under the MIT License.

import unittest
import azure_cosmoscx


class TestEngineVersion(unittest.TestCase):
    def test_engine_version(self):
        self.assertRegex(azure_cosmoscx.version(), r"\d+\.\d+\.\d+")
