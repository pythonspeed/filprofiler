"""End-to-end tests."""

from subprocess import check_call, check_output, CalledProcessError, run, PIPE
from tempfile import mkdtemp, NamedTemporaryFile
from pathlib import Path
import os
import time
import sys
from typing import Union
import re
import shutil

import numpy.core.numeric
from pampy import match, _ as ANY
import pytest
import psutil

from filprofiler._testing import get_allocations, big, as_mb
from filprofiler._utils import glibc_version

TEST_SCRIPTS = Path("tests") / "test-scripts"


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


def test_threaded_allocation_tracking():
    """
    fil-profile tracks allocations from all threads.

    1. The main thread gets profiled.
    2. Other threads get profiled.
    """
    script = TEST_SCRIPTS / "threaded.py"
    output_dir = profile(script)
    allocations = get_allocations(output_dir)

    import threading

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
    script = TEST_SCRIPTS / "threaded_aftermain.py"
    output_dir = profile(script)
    allocations = get_allocations(output_dir)

    import threading

    threading = (threading.__file__, "run", ANY)
    ones = (numpy.core.numeric.__file__, "ones", ANY)
    script = str(script)
    thread1_path1 = ((script, "thread1", 9), ones)

    assert match(allocations, {thread1_path1: big}, as_mb) == pytest.approx(70, 0.1)


def test_c_thread():
    """
    Allocations in C-only threads are considered allocations by the Python code
    that launched the thread.
    """
    script = TEST_SCRIPTS / "c-thread.py"
    output_dir = profile(script)
    allocations = get_allocations(output_dir)

    script = str(script)
    alloc = ((script, "<module>", 13), (script, "main", 9))

    assert match(allocations, {alloc: big}, as_mb) == pytest.approx(17, 0.1)


def test_malloc_in_c_extension():
    """
    Various malloc() and friends variants in C extension gets captured.
    """
    script = TEST_SCRIPTS / "malloc.py"
    output_dir = profile(script, "--size", "70")
    allocations = get_allocations(output_dir)

    script = str(script)

    # The realloc() in the scripts adds 10 to the 70:
    path = ((script, "<module>", 32), (script, "main", 28))
    assert match(allocations, {path: big}, as_mb) == pytest.approx(70 + 10, 0.1)

    # The C++ new allocation:
    path = ((script, "<module>", 32), (script, "main", 23))
    assert match(allocations, {path: big}, as_mb) == pytest.approx(40, 0.1)

    # C++ aligned_alloc(); not available on Conda, where it's just a macro
    # redirecting to posix_memalign.
    if not os.environ.get("CONDA_PREFIX"):
        path = ((script, "<module>", 32), (script, "main", 24))
        assert match(allocations, {path: big}, as_mb) == pytest.approx(90, 0.1)

    # Py*_*Malloc APIs:
    path = ((script, "<module>", 32), (script, "main", 25))
    assert match(allocations, {path: big}, as_mb) == pytest.approx(30, 0.1)

    # posix_memalign():
    path = ((script, "<module>", 32), (script, "main", 26))
    assert match(allocations, {path: big}, as_mb) == pytest.approx(15, 0.1)


def test_anonymous_mmap():
    """
    Non-file-backed mmap() gets detected and tracked.

    (NumPy uses Python memory APIs, so is not sufficient to test this.)
    """
    script = TEST_SCRIPTS / "mmaper.py"
    output_dir = profile(script)
    allocations = get_allocations(output_dir)

    script = str(script)
    path = ((script, "<module>", 6),)

    assert match(allocations, {path: big}, as_mb) == pytest.approx(60, 0.1)


def test_python_objects():
    """
    Python objects gets detected and tracked.

    (NumPy uses Python memory APIs, so is not sufficient to test this.)
    """
    script = TEST_SCRIPTS / "pyobject.py"
    output_dir = profile(script)
    allocations = get_allocations(output_dir)

    script = str(script)
    path = ((script, "<module>", 1),)
    path2 = ((script, "<module>", 8), (script, "<genexpr>", 8))

    assert match(allocations, {path: big}, as_mb) == pytest.approx(34, 1)
    assert match(allocations, {path2: big}, as_mb) == pytest.approx(46, 1)


def test_minus_m():
    """
    `fil-profile -m package` runs the package.
    """
    dir = TEST_SCRIPTS
    script = (dir / "malloc.py").absolute()
    output_dir = profile("-m", "malloc", "--size", "50", cwd=dir)
    allocations = get_allocations(output_dir)
    stripped_allocations = {k[3:]: v for (k, v) in allocations.items()}
    script = str(script)
    path = ((script, "<module>", 32), (script, "main", 28))

    assert match(stripped_allocations, {path: big}, as_mb) == pytest.approx(
        50 + 10, 0.1
    )


def test_minus_m_minus_m():
    """
    `python -m filprofiler -m package` runs the package.
    """
    dir = TEST_SCRIPTS
    script = (dir / "malloc.py").absolute()
    output_dir = Path(mkdtemp())
    check_call(
        [
            sys.executable,
            "-m",
            "filprofiler",
            "-o",
            str(output_dir),
            "run",
            "-m",
            "malloc",
            "--size",
            "50",
        ],
        cwd=dir,
    )
    allocations = get_allocations(output_dir)
    stripped_allocations = {k[3:]: v for (k, v) in allocations.items()}
    script = str(script)
    path = ((script, "<module>", 32), (script, "main", 28))

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
        assert b"\nLD_PRELOAD=" not in result.splitlines()
        # Not actually done at the moment, though perhaps it should be:
        # assert b"DYLD_INSERT_LIBRARIES" not in result


def test_out_of_memory():
    """
    If an allocation is run that runs out of memory, current allocations are
    written out.
    """
    script = TEST_SCRIPTS / "oom.py"
    output_dir = profile(script, expect_exit_code=53)
    time.sleep(10)  # wait for child process to finish
    allocations = get_allocations(
        output_dir,
        [
            "out-of-memory.svg",
            "out-of-memory-reversed.svg",
            "out-of-memory.prof",
        ],
        "out-of-memory.prof",
    )

    ones = (numpy.core.numeric.__file__, "ones", ANY)
    script = str(script)
    expected_small_alloc = ((script, "<module>", 9), ones)
    toobig_alloc = ((script, "<module>", 12), ones)

    assert match(allocations, {expected_small_alloc: big}, as_mb) == pytest.approx(
        100, 0.1
    )
    assert match(allocations, {toobig_alloc: big}, as_mb) == pytest.approx(
        1024 * 1024 * 1024, 0.1
    )


def test_out_of_memory_slow_leak():
    """
    If an allocation is run that runs out of memory slowly, current allocations are
    written out.
    """
    script = TEST_SCRIPTS / "oom-slow.py"
    output_dir = profile(script, expect_exit_code=53)
    time.sleep(10)  # wait for child process to finish
    allocations = get_allocations(
        output_dir,
        [
            "out-of-memory.svg",
            "out-of-memory-reversed.svg",
            "out-of-memory.prof",
        ],
        "out-of-memory.prof",
    )

    expected_alloc = ((str(script), "<module>", 3),)

    # Should've allocated at least a little before running out, unless testing
    # environment is _really_ restricted, in which case other tests would've
    # failed.
    assert match(allocations, {expected_alloc: big}, as_mb) > 100


def get_systemd_run_args(available_memory):
    """
    Figure out if we're on system with cgroups v2, or not, and return
    appropriate systemd-run args.

    If we don't have v2, we'll need to be root, unfortunately.
    """
    args = [
        "systemd-run",
        "--uid",
        str(os.geteuid()),
        "--gid",
        str(os.getegid()),
        "-p",
        f"MemoryLimit={available_memory // 2}B",
    ]
    try:
        check_call(args + ["--user", "printf", "hello"])
        args += ["--user", "--scope"]
    except CalledProcessError:
        # cgroups v1 doesn't do --user :(
        args = ["sudo", "--preserve-env=PATH"] + args + ["-t", "--same-dir"]
    return args


@pytest.mark.skipif(
    shutil.which("systemd-run") is None or glibc_version() < (2, 30),
    reason="systemd-run not found, or old systemd probably",
)
def test_out_of_memory_slow_leak_cgroups():
    """
    If an allocation is run that runs out of memory slowly, hitting a cgroup
    limit that's lower than system memory, current allocations are written out.
    """
    available_memory = psutil.virtual_memory().available
    script = TEST_SCRIPTS / "oom-slow.py"
    output_dir = profile(
        script,
        expect_exit_code=53,
        argv_prefix=get_systemd_run_args(available_memory),
    )
    time.sleep(10)  # wait for child process to finish
    allocations = get_allocations(
        output_dir,
        [
            "out-of-memory.svg",
            "out-of-memory-reversed.svg",
            "out-of-memory.prof",
        ],
        "out-of-memory.prof",
    )

    expected_alloc = ((str(script), "<module>", 3),)

    # Should've allocated at least a little before running out, unless testing
    # environment is _really_ restricted, in which case other tests would've
    # failed.
    assert match(allocations, {expected_alloc: big}, as_mb) > 100


def test_external_behavior():
    """
    1. Stdout and stderr from the code is printed normally.
    2. Fil only adds stderr lines prefixed with =fil-profile=
    3. A browser is launched with file:// URL pointing to an HTML file.
    """
    script = TEST_SCRIPTS / "printer.py"
    env = os.environ.copy()
    f = NamedTemporaryFile("r+")
    # A custom "browser" that just writes the URL to a file:
    env["BROWSER"] = "{} %s {}".format(TEST_SCRIPTS / "write-to-file.py", f.name)
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
    no_args_minus_m = run(
        [sys.executable, "-m", "filprofiler"], stdout=PIPE, stderr=PIPE
    )
    assert no_args.returncode == with_help.returncode
    assert no_args.stdout == with_help.stdout
    assert no_args.stderr == with_help.stderr
    assert no_args_minus_m.stdout == with_help.stdout
    assert no_args_minus_m.stderr == with_help.stderr


def test_fortran():
    """
    Fil can capture Fortran allocations.
    """
    script = TEST_SCRIPTS / "fortranallocate.py"
    output_dir = profile(script)
    allocations = get_allocations(output_dir)

    script = str(script)
    path = ((script, "<module>", 3),)

    assert match(allocations, {path: big}, as_mb) == pytest.approx(40, 0.1)


def test_free():
    """free() frees allocations as far as Fil is concerned."""
    script = TEST_SCRIPTS / "ldpreload.py"
    profile(script)


def test_interpreter_with_fil():
    """Run tests that require `fil-profile python`."""
    check_call(
        [
            "fil-profile",
            "python",
            "-m",
            "pytest",
            str(TEST_SCRIPTS / "fil-interpreter.py"),
        ]
    )


def test_jupyter(tmpdir):
    """Jupyter magic can run Fil."""
    shutil.copyfile(TEST_SCRIPTS / "jupyter.ipynb", tmpdir / "jupyter.ipynb")
    check_call(
        [
            "jupyter",
            "nbconvert",
            "--execute",
            "jupyter.ipynb",
            "--to",
            "html",
        ],
        cwd=tmpdir,
    )
    output_dir = tmpdir / "fil-result"

    # IFrame with SVG was included in output:
    with open(tmpdir / "jupyter.html") as f:
        html = f.read()
    assert "<iframe" in html
    [svg_path] = re.findall(r'src="([^"]*\.svg)"', html)
    assert svg_path.endswith("peak-memory.svg")
    assert Path(tmpdir / svg_path).exists()

    # Allocations were tracked:
    allocations = get_allocations(output_dir)
    print(allocations)
    path = (
        (re.compile("<ipython-input-3-.*"), "__magic_run_with_fil", 3),
        (re.compile("<ipython-input-2-.*"), "alloc", 4),
        (numpy.core.numeric.__file__, "ones", ANY),
    )
    assert match(allocations, {path: big}, as_mb) == pytest.approx(48, 0.1)
    path2 = (
        (re.compile("<ipython-input-3-.*"), "__magic_run_with_fil", 2),
        (numpy.core.numeric.__file__, "ones", ANY),
    )
    assert match(allocations, {path2: big}, as_mb) == pytest.approx(20, 0.1)
    # It's possible to run nbconvert again.
    check_call(
        [
            "jupyter",
            "nbconvert",
            "--execute",
            "jupyter.ipynb",
            "--to",
            "html",
        ],
        cwd=tmpdir,
    )


def test_no_threadpools_filprofile_run():
    """`fil-profile run` disables thread pools it knows about."""
    check_call(
        [
            "fil-profile",
            "run",
            str(TEST_SCRIPTS / "threadpools.py"),
        ]
    )


def test_malloc_on_thread_exit():
    """malloc() in thread shutdown handler doesn't blow things up.

    Reproducer for https://github.com/pythonspeed/filprofiler/issues/99
    """
    check_call(
        [
            "fil-profile",
            "run",
            str(TEST_SCRIPTS / "thread_exit.py"),
        ]
    )


def test_api_import(tmpdir):
    """
    It should be possible to import filprofiler.api even when NOT running under
    Fil.
    """
    # Importing is fine:
    from filprofiler import api

    # Calling APIs won't work:
    with pytest.raises(RuntimeError):
        api.profile(lambda: None, tmpdir)
