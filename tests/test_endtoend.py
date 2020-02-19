"""End-to-end tests."""

from subprocess import check_call, check_output
from tempfile import mkdtemp, NamedTemporaryFile
from pathlib import Path

import pytest


def get_allocations(output_directory: Path):
    """Parses peak-memory.prof, returns mapping from callstack to size in KiB."""
    result = {}
    with open(output_directory / "peak-memory.prof") as f:
        for line in f:
            *calls, size_kb = line.split(" ")
            calls = " ".join(calls)
            size_kb = int(size_kb)
            result[tuple(calls.split(";"))] = size_kb
    return result


def profile(script: Path, *arguments: str) -> Path:
    """Run fil-profile on given script, return path to output directory."""
    output = Path(mkdtemp())
    check_call(["fil-profile", "-o", str(output), str(script)] + list(arguments))
    return output


def test_threaded_allocation_tracking():
    """
    fil-profile tracks allocations from all threads.

    1. The main thread gets profiled.
    2. Other threads get profiled.
    """
    script = Path("python-benchmarks") / "threaded.py"
    output_dir = profile(script)
    allocations = get_allocations(output_dir)

    import threading
    import numpy.core.numeric

    threading = threading.__file__ + ":run"
    ones = numpy.core.numeric.__file__ + ":ones"
    script = str(script)
    h = script + ":h"

    # The main thread:
    main_path = (script + ":<module>", script + ":main", h, ones)
    assert allocations[main_path] / 1024 == pytest.approx(50, 0.1)

    # Thread that ends before main thread:
    thread1_path1 = (threading, script + ":thread1", script + ":child1", h, ones)
    assert allocations[thread1_path1] / 1024 == pytest.approx(30, 0.1)
    thread1_path2 = (threading, script + ":thread1", h, ones)
    assert allocations[thread1_path2] / 1024 == pytest.approx(20, 0.1)


def test_thread_allocates_after_main_thread_is_done():
    """
    fil-profile tracks thread allocations that happen after the main thread
    exits.
    """
    script = Path("python-benchmarks") / "threaded_aftermain.py"
    output_dir = profile(script)
    allocations = get_allocations(output_dir)

    import threading
    import numpy.core.numeric

    threading = threading.__file__ + ":run"
    ones = numpy.core.numeric.__file__ + ":ones"
    script = str(script)
    thread1_path1 = (threading, script + ":thread1", ones)
    assert allocations[thread1_path1] / 1024 == pytest.approx(70, 0.1)


def test_ld_preload_disabled_for_subprocesses():
    """
    LD_PRELOAD is reset so subprocesses don't get the malloc() preload.
    """
    with NamedTemporaryFile() as script_file:
        script_file.write(
            b"""\
import subprocess
print(subprocess.check_output(["env"]))
"""
        )
        script_file.flush()
        result = check_output(["fil-profile", "-o", mkdtemp(), str(script_file.name)])
        assert b"LD_PRELOAD" not in result
