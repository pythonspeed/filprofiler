"""Tests that need to be run under `fil-profile python`.

To run:

$ fil-profile python -m pytest python-benchmarks/fil-interpreter.py
"""

import sys

from ctypes import c_void_p

from filprofiler._tracer import preload

from pymalloc import pymalloc, pyfree


def test_no_profiling_by_default():
    """Neither memory tracking nor Python profiling happen by default."""
    address = pymalloc(365)
    # No information about size available, since it's not tracked:
    assert preload.pymemprofile_get_allocation_size(c_void_p(address)) == 0
    assert sys.getprofile() is None
