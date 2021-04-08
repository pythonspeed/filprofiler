"""Utility functions for testing."""

import os
from glob import glob
from pathlib import Path


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
    subdir = glob(str(output_directory / "*"))[0]
    assert sorted(os.listdir(subdir)) == sorted(expected_files)
    for expected_file in expected_files:
        assert (Path(subdir) / expected_file).stat().st_size > 0
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


def as_mb(*args):
    """Convert last argument from kilobyte to megabyte."""
    return args[-1] / 1024


def big(length):
    """Return True for large values."""
    return length > 10000
