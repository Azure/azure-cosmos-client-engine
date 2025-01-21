import unittest
import azure_cosmoscx

class TestEngineVersion(unittest.TestCase):
    def test_engine_version(self):
        self.assertRegex(azure_cosmoscx.engine_version(), r"\d+\.\d+\.\d+")