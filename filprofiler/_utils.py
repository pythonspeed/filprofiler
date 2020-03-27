"""Utilities."""

from importlib.util import find_spec


def library_path(name):
    """Return the path of a shared library."""
    return find_spec("filprofiler." + name).origin
