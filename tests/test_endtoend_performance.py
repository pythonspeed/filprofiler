"""End-to-end tests for performance profiling."""

from pathlib import Path
import threading
import re
from subprocess import check_call

from pampy import match, _ as ANY
import pytest

from filprofiler._testing import get_performance_samples, profile, RUNNING, WAITING

TEST_SCRIPTS = Path("tests") / "test-scripts" / "performance"

THREAD = re.compile(r"\[Thread \d*\]")


def run(filename: str):
    """Return tuple (script_path, samples)."""
    script = TEST_SCRIPTS / filename
    output_dir = profile(script)
    samples = get_performance_samples(output_dir)

    script = str(script)
    return script, samples


def test_minimal():
    """
    Minimal test of performance sampling CPU-bound Python program.
    """
    script, samples = run("minimal.py")
    path = (THREAD, (script, "<module>", 12), (script, "calc", 8), RUNNING)
    assert match(samples, {path: ANY}, lambda *x: x[-1]) == pytest.approx(1.0, 0.1)


def test_threads():
    """
    Performance profiling captures Python threads only.

    C threads are not included; insofar as they are used from Python code, they
    will show up as Python threads waiting for a result.  If they're not used
    from Python, they are likely not doing anything interesting, e.g. a
    threadpool waiting for work.
    """
    script, samples = run("threads.py")

    # 4 threads:
    # - Main thread ~1 second, mostly sleeping
    # - Thread 1, ~1 second of running
    # - Thread 2, ~0.5 seconds of sleeping
    # - Thread 3, C code, ~1 seconds of sleeping. Not included!
    # Total: 2.5 seconds

    path_main = (
        THREAD,
        (script, "<module>", 32),
        (threading.__file__, "join", ANY),
        ANY,
        WAITING,
    )
    assert match(samples, {path_main: ANY}, lambda *x: x[-1]) == pytest.approx(
        1 / 2.5, 0.1
    )

    path_1 = (THREAD, (script, "thread1", 18), (script, "calc", 12), RUNNING)
    assert match(samples, {path_1: ANY}, lambda *x: x[-1]) == pytest.approx(
        1 / 2.5, 0.1
    )

    path_2 = (THREAD, (script, "thread2", 22), WAITING)
    assert match(samples, {path_2: ANY}, lambda *x: x[-1]) == pytest.approx(
        0.5 / 2.5, 0.1
    )


def test_thread_after_exit():
    """
    Performance profiling captures thread code that runs after main thread
    exits.
    """
    script, samples = run("thread_after_main.py")

    # 2 threads:
    # - Main: 0.25 sleeping, 0.25 waiting for thread 1 to shutdown
    # - Thread 1: 0.5 waiting
    # Total: 0.75

    path_main_sleep = (
        THREAD,
        (script, "<module>", 12),
        WAITING,
    )
    assert match(samples, {path_main_sleep: ANY}, lambda *x: x[-1]) == pytest.approx(
        0.25, 0.1
    )

    # Python implementation detail...
    path_main_wait = (
        THREAD,
        (threading.__file__, ANY, ANY),
        WAITING,
    )
    assert match(samples, {path_main_wait: ANY}, lambda *x: x[-1]) == pytest.approx(
        0.25, 0.1
    )

    path_1 = (THREAD, (script, "sleepy", 8), WAITING)
    assert match(samples, {path_1: ANY}, lambda *x: x[-1]) == pytest.approx(0.5, 0.1)


def test_api():
    """
    Performance profiling can be enabled via the API.
    """
    check_call(
        [
            "fil-profile",
            "python",
            "-m",
            "pytest",
            str(TEST_SCRIPTS / "interpreter.py"),
        ]
    )


def test_gil():
    """
    Performance profiling can capture situations where threads are fighting
    over the GIL.
    """
    script, samples = run("gil.py")

    calc = ((script, "go", 14), (script, "calc", 8))
    running = calc + (RUNNING,)
    waiting = calc + (WAITING,)
    main_running = (
        THREAD,
        (script, "<module>", 18),
    ) + running
    main_waiting = (
        THREAD,
        (script, "<module>", 18),
    ) + waiting

    assert match(
        samples, {(THREAD,) + running: ANY}, lambda *x: x[-1]
    ) == pytest.approx(0.25, 0.5)
    assert match(
        samples, {(THREAD,) + waiting: ANY}, lambda *x: x[-1]
    ) == pytest.approx(0.25, 0.5)
    assert match(samples, {main_running: ANY}, lambda *x: x[-1]) == pytest.approx(
        0.25, 0.5
    )
    assert match(samples, {main_waiting: ANY}, lambda *x: x[-1]) == pytest.approx(
        0.25, 0.5
    )