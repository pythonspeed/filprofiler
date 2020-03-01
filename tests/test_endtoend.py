"""End-to-end tests."""

from subprocess import check_call, check_output
from tempfile import mkdtemp, NamedTemporaryFile
from pathlib import Path

from pampy import match, _ as ANY
import pytest


def get_allocations(output_directory: Path):
    """Parses peak-memory.prof, returns mapping from callstack to size in KiB."""
    result = {}
    with open(output_directory / "peak-memory.prof") as f:
        for line in f:
            *calls, size_kb = line.split(" ")
            calls = " ".join(calls)
            size_kb = int(int(size_kb) / 1024)
            path = []
            if calls == "[No Python stack]":
                result[calls] = size_kb
                continue
            for call in calls.split(";"):
                part1, func_name = call.rsplit(" ", 1)
                assert func_name[0] == "("
                assert func_name[-1] == ")"
                func_name = func_name[1:-1]
                file_name, line = part1.split(":")
                line = int(line)
                path.append((file_name, func_name, line))
            result[tuple(path)] = size_kb
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

    threading = (threading.__file__, "run", ANY)
    ones = (numpy.core.numeric.__file__, "ones", ANY)
    script = str(script)
    h = (script, "h", 7)

    # The main thread:
    main_path = ((script, "<module>", 24), (script, "main", 21), h, ones)

    def as_mb(*args):
        return args[-1] / 1024

    def big(length):
        return length > 10000

    assert match(allocations, {main_path: big}, as_mb) == pytest.approx(50, 0.1)

    # Thread that ends before main thread:
    thread1_path1 = (
        threading,
        (script, "thread1", 15),
        (script, "child1", 10),
        h,
        ones,
    )
    assert match(allocations, {thread1_path1: big}, as_mb) == pytest.approx(30, 0.1)
    thread1_path2 = (threading, (script, "thread1", 13), h, ones)
    assert match(allocations, {thread1_path2: big}, as_mb) == pytest.approx(20, 0.1)


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

    threading = (threading.__file__, "run", ANY)
    ones = (numpy.core.numeric.__file__, "ones", ANY)
    script = str(script)
    thread1_path1 = (threading, (script, "thread1", 9), ones)

    def as_mb(*args):
        return args[-1] / 1024

    def big(length):
        return length > 10000

    assert match(allocations, {thread1_path1: big}, as_mb) == pytest.approx(70, 0.1)


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
