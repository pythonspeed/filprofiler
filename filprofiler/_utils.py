"""Utilities."""

from importlib.util import find_spec
from datetime import datetime


def library_path(name):
    """Return the path of a shared library."""
    return find_spec("filprofiler." + name).origin


def timestamp_now() -> str:
    """Return current time as a string."""
    now = datetime.now()
    return now.isoformat(timespec="milliseconds")
