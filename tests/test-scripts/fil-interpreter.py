"""Tests that need to be run under `fil-profile python`.

To run:

$ fil-profile python -m pytest tests/test-scripts/fil-interpreter.py
"""

import sys
import os
from ctypes import c_void_p
import re
from pathlib import Path
from subprocess import check_output
import multiprocessing

import pytest
import numpy as np
import numpy.core.numeric
from pampy import _ as ANY, match
from IPython.core.displaypub import CapturingDisplayPublisher
from IPython.core.interactiveshell import InteractiveShell
import threadpoolctl

from filprofiler._tracer import (
    preload,
    start_tracing,
    stop_tracing,
    disable_thread_pools,
)
from filprofiler._testing import get_allocations, big, as_mb
from filprofiler._ipython import run_with_profile
from filprofiler.api import profile
from pymalloc import pymalloc


def test_no_profiling():
    """Neither memory tracking nor Python profiling happen by default."""
    address = pymalloc(365)
    # No information about size available, since it's not tracked:
    assert preload.pymemprofile_get_allocation_size(c_void_p(address)) == 0
    assert sys.getprofile() is None


def test_temporary_profiling(tmpdir):
    """Profiling can be run temporarily."""
    # get_allocations() expects actual output in a subdirectory.
    def f():
        arr = np.ones((1024, 1024, 4), dtype=np.uint64)  # 32MB
        del arr
        return 1234

    result = profile(f, tmpdir / "output")
    assert result == 1234

    # Allocations were tracked:
    path = ((__file__, "f", 48), (numpy.core.numeric.__file__, "ones", ANY))
    allocations = get_allocations(tmpdir)
    assert match(allocations, {path: big}, as_mb) == pytest.approx(32, 0.1)

    # Profiling stopped:
    test_no_profiling()


def run_in_ipython_shell(code_cells):
    """Run a list of strings in IPython.

    Returns parsed allocations.
    """
    InteractiveShell.clear_instance()

    shell = InteractiveShell.instance(display_pub_class=CapturingDisplayPublisher)
    for code in code_cells:
        shell.run_cell(code)
    InteractiveShell.clear_instance()
    html = shell.display_pub.outputs[-1]["data"]["text/html"]
    assert "<iframe" in html
    [svg_path] = re.findall('src="([^"]*)"', html)
    assert svg_path.endswith("peak-memory.svg")
    resultdir = Path(svg_path).parent.parent

    return get_allocations(resultdir)


def test_ipython_profiling(tmpdir):
    """Profiling can be run via IPython magic."""
    cwd = os.getcwd()
    os.chdir(tmpdir)
    allocations = run_in_ipython_shell(
        [
            "%load_ext filprofiler",
            """\
%%filprofile
import numpy as np
arr = np.ones((1024, 1024, 4), dtype=np.uint64)  # 32MB
""",
        ]
    )

    # Allocations were tracked:
    path = (
        (re.compile("<ipython-input-1-.*"), "__magic_run_with_fil", 3),
        (numpy.core.numeric.__file__, "ones", ANY),
    )
    assert match(allocations, {path: big}, as_mb) == pytest.approx(32, 0.1)

    # Profiling stopped:
    test_no_profiling()


def test_ipython_exception_while_profiling(tmpdir):
    """
    Profiling can be run via IPython magic, still profiles and shuts down
    correctly on an exception.

    This will log a RuntimeError. That is expected.
    """
    cwd = os.getcwd()
    os.chdir(tmpdir)
    allocations = run_in_ipython_shell(
        [
            "%load_ext filprofiler",
            """\
%%filprofile
import numpy as np
arr = np.ones((1024, 1024, 2), dtype=np.uint64)  # 16MB
raise RuntimeError("The test will log this, it's OK.")
arr = np.ones((1024, 1024, 8), dtype=np.uint64)  # 64MB
""",
        ]
    )

    # Allocations were tracked:
    path = (
        (re.compile("<ipython-input-1-.*"), "__magic_run_with_fil", 3),
        (numpy.core.numeric.__file__, "ones", ANY),
    )
    assert match(allocations, {path: big}, as_mb) == pytest.approx(16, 0.1)

    # Profiling stopped:
    test_no_profiling()


def test_ipython_non_standard_indent(tmpdir):
    """
    Profiling can be run via IPython magic, still profiles and shuts down
    correctly on an exception.

    This will log a RuntimeError. That is expected.
    """
    cwd = os.getcwd()
    os.chdir(tmpdir)
    allocations = run_in_ipython_shell(
        [
            "%load_ext filprofiler",
            """\
%%filprofile
import numpy as np
def f():  # indented with 5 spaces what
     arr = np.ones((1024, 1024, 2), dtype=np.uint64)  # 16MB
f()
""",
        ]
    )

    # Allocations were tracked:
    path = (
        (re.compile("<ipython-input-1-.*"), "__magic_run_with_fil", 5),
        (re.compile("<ipython-input-1-.*"), "f", 4),
        (numpy.core.numeric.__file__, "ones", ANY),
    )
    assert match(allocations, {path: big}, as_mb) == pytest.approx(16, 0.1)

    # Profiling stopped:
    test_no_profiling()


@pytest.mark.parametrize(
    "profile_func", [lambda f, tempdir: run_with_profile(f), profile,]
)
def test_profiling_disables_threadpools(tmpdir, profile_func):
    """
    Memory profiling disables thread pools, then restores them when done.
    """
    cwd = os.getcwd()
    os.chdir(tmpdir)

    import numexpr
    import blosc

    numexpr.set_num_threads(3)
    blosc.set_nthreads(3)
    with threadpoolctl.threadpool_limits(3, "blas"):

        def check():
            assert numexpr.set_num_threads(2) == 1
            assert blosc.set_nthreads(2) == 1

            for d in threadpoolctl.threadpool_info():
                assert d["num_threads"] == 1, d

        profile_func(check, tmpdir)

        # Resets when done:
        assert numexpr.set_num_threads(2) == 3
        assert blosc.set_nthreads(2) == 3

        for d in threadpoolctl.threadpool_info():
            if d["user_api"] == "blas":
                assert d["num_threads"] == 3, d


def test_profiling_without_blosc_and_numexpr(tmpdir):
    """
    The support for numexpr and blosc is optional; disabling them should work
    even when they're not present.
    """
    import sys

    sys.modules["blosc"] = None
    sys.modules["numexpr"] = None
    try:
        with disable_thread_pools():
            pass
    finally:
        del sys.modules["blosc"]
        del sys.modules["numexpr"]


def test_subprocess(tmpdir):
    """
    Running a subprocess doesn't blow up.
    """
    start_tracing(tmpdir)
    try:
        output = check_output(["printf", "hello"])
    finally:
        stop_tracing(tmpdir)
    assert output == b"hello"


def return123():
    return 123


@pytest.mark.parametrize("mode", ["spawn", "forkserver", "fork"])
def test_multiprocessing(tmpdir, mode):
    """
    Running a subprocess via multiprocessing in the various different modes
    doesn't blow up.
    """
    # Non-tracing:
    with multiprocessing.get_context(mode).Pool() as pool:
        assert pool.apply((3).__add__, (4,)) == 7

    # Tracing:
    start_tracing(tmpdir)
    try:
        with multiprocessing.get_context(mode).Pool() as pool:
            assert pool.apply((3).__add__, (4,)) == 7
    finally:
        stop_tracing(tmpdir)
