"""
Tests that run with ``filprofiler run -m pytest``.
"""
import re
import time

from pampy import match, _ as ANY
import pytest

from filprofiler.api import profile
from filprofiler._testing import get_performance_samples, RUNNING, WAITING

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
    path = (THREAD, (__file__, "f", 22), RUNNING)
    assert match(samples, {path: ANY}, lambda x: x) == pytest.approx(0.33, 0.2)
    path2 = (THREAD, (__file__, "f", 23), WAITING)
    assert match(samples, {path2: ANY}, lambda x: x) == pytest.approx(0.66, 0.2)
