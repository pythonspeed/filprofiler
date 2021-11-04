"""Utility functions for testing."""

import os
import re
from glob import glob
from pathlib import Path
from tempfile import mkdtemp
from subprocess import check_call, CalledProcessError
from typing import Union

RUNNING = "\u2BC8 Running"
WAITING = "\u29D7 Waiting"
WAITING2 = "\u29D7 Uninterruptible wait"
NO_PYTHON_STACK = "[No Python stack]"
OTHER = "? Other"


def parse_prof(path: Path):
    """Parses peak-memory.prof, returns iterable of (path, samples)."""
    with open(path) as f:
        for line in f:
            *calls, samples = line.split(" ")
            calls = " ".join(calls)
            path = []
            if calls == NO_PYTHON_STACK:
                yield (calls, samples)
                continue
            for call in calls.split(";"):
                if (
                    call
                    in (
                        RUNNING,
                        WAITING,
                        WAITING2,
                        NO_PYTHON_STACK,
                        OTHER,
                    )
                    or call.startswith("[Thread ")
                ):
                    path.append(call)
                    continue
                part1, func_name = call.rsplit(" ", 1)
                assert func_name[0] == "("
                assert func_name[-1] == ")"
                func_name = func_name[1:-1]
                file_name, line = part1.split(":")
                line = int(line)
                path.append((file_name, func_name, line))
            yield (tuple(path), samples)


def get_prof_file(output_directory: Path, prof_file: str, expected_files=None) -> Path:
    """
    Return path to a .prof file.
    """
    if expected_files is None:
        expected_files = [
            "peak-memory.svg",
            "peak-memory-reversed.svg",
            "index.html",
            "peak-memory.prof",
            "performance.svg",
            "performance-reversed.svg",
            "performance.prof",
        ]
    subdir = glob(str(output_directory / "*"))[0]
    assert set(os.listdir(subdir)) >= set(expected_files)
    for expected_file in expected_files:
        assert (Path(subdir) / expected_file).stat().st_size > 0
    return glob(str(output_directory / "*" / prof_file))[0]


def get_allocations(
    output_directory: Path,
    expected_files=[
        "peak-memory.svg",
        "peak-memory-reversed.svg",
        "index.html",
        "peak-memory.prof",
    ],
    prof_file="peak-memory.prof",
    direct=False,
):
    """Parses peak-memory.prof, returns mapping from callstack to size in KiB."""
    if direct:
        prof_path = str(output_directory)
    else:
        prof_path = get_prof_file(output_directory, prof_file, expected_files)

    result = {}
    for path, size_bytes in parse_prof(prof_path):
        size_kb = int(int(size_bytes) / 1024)
        if size_kb > 900:
            result[path] = size_kb
    return result


def get_performance_samples(output_directory: Path):
    """Parses performance.prof, returns mapping from callstack to % samples."""
    prof_path = get_prof_file(output_directory, "performance.prof")

    result = dict(
        (path, int(samples.strip())) for (path, samples) in parse_prof(prof_path)
    )
    total = sum(result.values())
    for key in result:
        result[key] /= total
    return result


def as_mb(*args):
    """Convert last argument from kilobyte to megabyte."""
    return args[-1] / 1024


def big(length):
    """Return True for large values."""
    return length > 10000


def profile(
    *arguments: Union[str, Path], expect_exit_code=0, argv_prefix=(), **kwargs
) -> Path:
    """Run fil-profile on given script, return path to output directory."""
    output = Path(mkdtemp())
    try:
        check_call(
            list(argv_prefix)
            + ["fil-profile", "-o", str(output), "run"]
            + list(arguments),
            **kwargs,
        )
        exit_code = 0
    except CalledProcessError as e:
        exit_code = e.returncode
    assert exit_code == expect_exit_code

    return output


def run_in_ipython_shell(code_cells, filename):
    """Run a list of strings in IPython.

    Returns Path to top-level directory usable by ``get_allocations()`` or
    ``get_performance_samples()``.
    """
    from IPython.core.displaypub import CapturingDisplayPublisher
    from IPython.core.interactiveshell import InteractiveShell

    InteractiveShell.clear_instance()

    shell = InteractiveShell.instance(display_pub_class=CapturingDisplayPublisher)
    for code in code_cells:
        shell.run_cell(code)
    InteractiveShell.clear_instance()
    html = shell.display_pub.outputs[-1]["data"]["text/html"]
    assert "<iframe" in html
    svg_paths = re.findall('src="([^"]*)"', html)
    for svg_path in svg_paths:
        if svg_path.endswith(filename):
            return Path(svg_path).parent.parent
    assert False, f"Couldn't find {filename} in {svg_paths}"
