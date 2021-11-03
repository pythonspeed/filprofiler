"""
Tests that run with ``filprofiler run -m pytest``.
"""
import re
import time
import os

from pampy import match, _ as ANY
import pytest

from filprofiler.api import profile
from filprofiler._testing import (
    get_performance_samples,
    RUNNING,
    WAITING,
    run_in_ipython_shell,
)

THREAD = re.compile(r"\[Thread \d*\]")


def test_temporary_profiling(tmpdir):
    """Profiling can be run temporarily."""

    def f():
        start = time.time()
        while time.time() < start + 0.5:
            sum(range(10000))
        time.sleep(1)
        return 1234

    result = profile(f, tmpdir / "output")
    assert result == 1234

    # Performance was tracked
    samples = get_performance_samples(tmpdir)
    path = (THREAD, (__file__, "f", 28), RUNNING)
    assert match(samples, {path: ANY}, lambda x: x) == pytest.approx(0.33, 0.2)
    path2 = (THREAD, (__file__, "f", 29), WAITING)
    assert match(samples, {path2: ANY}, lambda x: x) == pytest.approx(0.66, 0.2)


def test_ipython_profiling(tmpdir):
    """Profiling can be run via IPython magic."""
    cwd = os.getcwd()
    os.chdir(tmpdir)
    samples = get_performance_samples(
        run_in_ipython_shell(
            [
                "%load_ext filprofiler",
                """\
%%filprofile
import time
start = time.time()
while time.time() < start + 0.5:
    sum(range(10000))
time.sleep(1)
""",
            ],
            "performance.svg",
        ),
    )

    # Performance was tracked:
    path = (
        THREAD,
        (re.compile("<ipython-input-1-.*"), "__magic_run_with_fil", 5),
        RUNNING,
    )
    assert match(samples, {path: ANY}, lambda x: x) == pytest.approx(0.3, 0.3)
