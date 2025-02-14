from . import _azure_cosmoscx
from .query_engine import QueryEngine


def version():
    return _azure_cosmoscx.version()


def enable_tracing():
    _azure_cosmoscx.enable_tracing()
