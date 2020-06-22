"""End-to-end tests."""

from subprocess import check_call, check_output, CalledProcessError, run, PIPE
from tempfile import mkdtemp, NamedTemporaryFile
from pathlib import Path
from glob import glob
import os
import time
import sys

from pampy import match, _ as ANY
import pytest


def get_allocations(
    output_directory: Path,
    expected_files=[
        "peak-memory.svg",
        "peak-memory-reversed.svg",
        "index.html",
        "peak-memory.prof",
    ],
    prof_file="peak-memory.prof",
):
    """Parses peak-memory.prof, returns mapping from callstack to size in KiB."""
    assert sorted(os.listdir(glob(str(output_directory / "*"))[0])) == sorted(
        expected_files
    )
    result = {}
    with open(glob(str(output_directory / "*" / prof_file))[0]) as f:
        for line in f:
            *calls, size_kb = line.split(" ")
            calls = " ".join(calls)
            size_kb = int(int(size_kb) / 1024)
            path = []
            if calls == "[No Python stack]":
                result[calls] = size_kb
                continue
            for call in calls.split(";"):
                if call.startswith("TB@@"):
                    continue
                part1, func_name = call.rsplit(" ", 1)
                assert func_name[0] == "("
                assert func_name[-1] == ")"
                func_name = func_name[1:-1]
                file_name, line = part1.split(":")
                line = int(line)
                path.append((file_name, func_name, line))
            if size_kb > 900:
                result[tuple(path)] = size_kb
    return result


def profile(*arguments: str, expect_exit_code=0, **kwargs) -> Path:
    """Run fil-profile on given script, return path to output directory."""
    output = Path(mkdtemp())
    try:
        check_call(
            ["fil-profile", "-o", str(output), "run"] + list(arguments), **kwargs
        )
        exit_code = 0
    except CalledProcessError as e:
        exit_code = e.returncode
    assert exit_code == expect_exit_code

    return output


def as_mb(*args):
    return args[-1] / 1024


def big(length):
    return length > 10000


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

    assert match(allocations, {main_path: big}, as_mb) == pytest.approx(50, 0.1)

    # Thread that ends before main thread:
    thread1_path1 = (
        (script, "thread1", 15),
        (script, "child1", 10),
        h,
        ones,
    )
    assert match(allocations, {thread1_path1: big}, as_mb) == pytest.approx(30, 0.1)
    thread1_path2 = ((script, "thread1", 13), h, ones)
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
    thread1_path1 = ((script, "thread1", 9), ones)

    assert match(allocations, {thread1_path1: big}, as_mb) == pytest.approx(70, 0.1)


def test_malloc_in_c_extension():
    """
    Direct malloc() in C extension gets captured.

    (NumPy uses Python memory APIs, so is not sufficient to test this.)
    """
    script = Path("python-benchmarks") / "malloc.py"
    output_dir = profile(script, "--size", "70")
    allocations = get_allocations(output_dir)

    script = str(script)
    path = ((script, "<module>", 21), (script, "main", 17))

    # The realloc() in the scripts adds 10 to the 70:
    assert match(allocations, {path: big}, as_mb) == pytest.approx(70 + 10, 0.1)


def test_minus_m():
    """
    `fil-profile -m package` runs the package.
    """
    dir = Path("python-benchmarks")
    script = (dir / "malloc.py").absolute()
    output_dir = profile("-m", "malloc", "--size", "50", cwd=dir)
    allocations = get_allocations(output_dir)
    stripped_allocations = {k[3:]: v for (k, v) in allocations.items()}
    script = str(script)
    path = ((script, "<module>", 21), (script, "main", 17))

    assert match(stripped_allocations, {path: big}, as_mb) == pytest.approx(
        50 + 10, 0.1
    )


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
        result = check_output(
            ["fil-profile", "-o", mkdtemp(), "run", str(script_file.name)]
        )
        assert b"LD_PRELOAD" not in result
        # Not actually done at the moment, though perhaps it should be:
        # assert b"DYLD_INSERT_LIBRARIES" not in result


def test_out_of_memory():
    """
    If an allocation is run that runs out of memory, current allocations are
    written out.
    """
    script = Path("python-benchmarks") / "oom.py"
    output_dir = profile(script, expect_exit_code=5)
    time.sleep(10)  # wait for child process to finish
    allocations = get_allocations(
        output_dir,
        ["out-of-memory.svg", "out-of-memory-reversed.svg", "out-of-memory.prof",],
        "out-of-memory.prof",
    )

    import threading
    import numpy.core.numeric

    ones = (numpy.core.numeric.__file__, "ones", ANY)
    script = str(script)
    expected_small_alloc = ((script, "<module>", 9), ones)
    toobig_alloc = ((script, "<module>", 14), ones)

    assert match(allocations, {expected_small_alloc: big}, as_mb) == pytest.approx(
        100, 0.1
    )
    assert match(allocations, {toobig_alloc: big}, as_mb) == pytest.approx(
        1024 * 1024 * 1024, 0.1
    )


def test_external_behavior():
    """
    1. Stdout and stderr from the code is printed normally.
    2. Fil only adds stderr lines prefixed with =fil-profile=
    3. A browser is launched with file:// URL pointing to an HTML file.
    """
    script = Path("python-benchmarks") / "printer.py"
    env = os.environ.copy()
    f = NamedTemporaryFile("r+")
    # A custom "browser" that just writes the URL to a file:
    env["BROWSER"] = "{} %s {}".format(
        Path("python-benchmarks") / "write-to-file.py", f.name
    )
    output_dir = Path(mkdtemp())
    result = run(
        ["fil-profile", "-o", str(output_dir), "run", str(script)],
        env=env,
        stdout=PIPE,
        stderr=PIPE,
        check=True,
        encoding=sys.getdefaultencoding(),
    )
    assert result.stdout == "Hello, world.\n"
    for line in result.stderr.splitlines():
        assert line.startswith("=fil-profile= ")
    url = f.read()
    assert url.startswith("file://")
    assert url.endswith(".html")
    assert os.path.exists(url[len("file://") :])


def test_no_args():
    """
    Running fil-profile with no arguments gives same result as --help.
    """
    no_args = run(["fil-profile"], stdout=PIPE, stderr=PIPE)
    with_help = run(["fil-profile", "--help"], stdout=PIPE, stderr=PIPE)
    assert no_args.returncode == with_help.returncode
    assert no_args.stdout == with_help.stdout
    assert no_args.stderr == with_help.stderr
