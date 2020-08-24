"""Tests that need to be run under `fil-profile python`.

To run:

$ fil-profile python -m pytest python-benchmarks/fil-interpreter.py
"""

import sys

from ctypes import c_void_p

import numpy as np
from pampy import _ as ANY, match
from filprofiler._tracer import preload, start_tracing, stop_tracing
from filprofiler._testing import get_allocations, big, as_mb
from pymalloc import pymalloc, pyfree


def test_no_profiling():
    """Neither memory tracking nor Python profiling happen by default."""
    address = pymalloc(365)
    # No information about size available, since it's not tracked:
    assert preload.pymemprofile_get_allocation_size(c_void_p(address)) == 0
    assert sys.getprofile() is None


def test_temporary_profiling(tmpdir):
    """Profiling can be run temporarily."""
    start_tracing(tmpdir)
    arr = np.ones((1024, 1024, 4), dtype=np.uint64)  # 32MB
    stop_tracing(tmpdir)

    # Allocations were tracked:
    import numpy.core.numeric

    path = ((__file__, "<module>", 30), (numpy.core.numeric.__file__, "ones", ANY))
    allocations = get_allocations(tmpdir)
    assert match(allocations, {path: big}, as_mb) == pytest.approx(32, 0.1)

    # Profiling stopped:
    test_no_profiling()
