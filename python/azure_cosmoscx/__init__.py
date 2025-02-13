from . import _azure_cosmoscx
from .query_engine import NativeQueryEngine


def version():
    return _azure_cosmoscx.version()
