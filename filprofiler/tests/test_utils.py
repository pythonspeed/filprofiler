"""Tests for filprofiler._utils."""

import os

from .._utils import library_path


def test_library_path():
    """The library is found and has the right suffix."""
    path = library_path("_filpreload")
    assert os.path.exists(path)
    assert path.endswith(".so")
