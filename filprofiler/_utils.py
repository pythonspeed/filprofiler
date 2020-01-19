"""Utilities."""

from os.path import abspath, dirname, join

def library_path(name):
    """Return the path of a shared library."""
    # TODO dylib on Macs.
    return join(dirname(abspath(__file__)), name + ".so")
