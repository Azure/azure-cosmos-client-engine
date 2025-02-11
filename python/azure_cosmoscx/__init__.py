import azure.cosmos.query_engine
from . import _azure_cosmoscx


def version():
    return _azure_cosmoscx.version()


class NativeQueryEngine(azure.cosmos.query_engine.QueryEngine):
    pass
