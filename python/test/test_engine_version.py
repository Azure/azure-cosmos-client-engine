import unittest
import azure_cosmoscx

class TestEngineVersion(unittest.TestCase):
    def test_engine_version(self):
        self.assertEqual(azure_cosmoscx.engine_version(), "0.2.0")