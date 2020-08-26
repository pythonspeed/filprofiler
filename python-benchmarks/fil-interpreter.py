"""Tests that need to be run under `fil-profile python`.

To run:

$ fil-profile python -m pytest python-benchmarks/fil-interpreter.py
"""

import sys
import os
from ctypes import c_void_p
import re
from pathlib import Path
import pytest
import numpy as np
from pampy import _ as ANY, match
from filprofiler._tracer import preload, start_tracing, stop_tracing
from filprofiler._testing import get_allocations, big, as_mb
from pymalloc import pymalloc
from IPython.core.displaypub import CapturingDisplayPublisher
from IPython.core.interactiveshell import InteractiveShell


def test_no_profiling():
    """Neither memory tracking nor Python profiling happen by default."""
    address = pymalloc(365)
    # No information about size available, since it's not tracked:
    assert preload.pymemprofile_get_allocation_size(c_void_p(address)) == 0
    assert sys.getprofile() is None


def test_temporary_profiling(tmpdir):
    """Profiling can be run temporarily."""
    start_tracing(tmpdir)

    def f():
        arr = np.ones((1024, 1024, 4), dtype=np.uint64)  # 32MB

    f()
    stop_tracing(tmpdir)

    # Allocations were tracked:
    import numpy.core.numeric

    path = ((__file__, "f", 36), (numpy.core.numeric.__file__, "ones", ANY))
    allocations = get_allocations(tmpdir)
    assert match(allocations, {path: big}, as_mb) == pytest.approx(32, 0.1)

    # Profiling stopped:
    test_no_profiling()


def test_ipython_profiling(tmpdir):
    """Profiling can be run via IPython magic."""
    cwd = os.getcwd()
    os.chdir(tmpdir)
    InteractiveShell.clear_instance()

    shell = InteractiveShell.instance(display_pub_class=CapturingDisplayPublisher)
    shell.run_cell("%load_ext filprofiler")
    shell.run_cell(
        """\
%%filprofile
import numpy as np
arr = np.ones((1024, 1024, 4), dtype=np.uint64)  # 32MB
"""
    )
    InteractiveShell.clear_instance()

    html = shell.display_pub.outputs[0]["data"]["text/html"]
    assert "iframe" in html
    [svg_path] = re.findall('src="([^"]*)"', html)
    assert svg_path.endswith("peak-memory.svg")
    resultdir = Path(svg_path).parent.parent

    # Allocations were tracked:
    import numpy.core.numeric

    path = (
        (re.compile("<ipython-input-1-.*"), "__magic_run_with_fil", 3),
        (numpy.core.numeric.__file__, "ones", ANY),
    )
    allocations = get_allocations(resultdir)
    assert match(allocations, {path: big}, as_mb) == pytest.approx(32, 0.1)

    # Profiling stopped:
    test_no_profiling()
