"""Trace code, so that libpymemprofile_api know's where we are."""

import atexit
from ctypes import PyDLL
from datetime import datetime
import os
import sys
import threading
import webbrowser
from contextlib import contextmanager
from pathlib import Path
from typing import Union
import traceback

from ._utils import timestamp_now, library_path
from ._report import render_report


def check_if_fil_preloaded():
    """Raise exception if Fil library is not preloaded."""
    from . import _original_pid

    if os.getenv("__FIL_STATUS") in ("launcher", None):
        raise RuntimeError(
            "Fil APIs can't be used from Python started without Fil "
            ", i.e. fil-profile on command-line, Fil kernel in Jupyter."
        )
    if os.getenv("__FIL_STATUS") == "subprocess" or os.getpid() != _original_pid:
        raise RuntimeError(
            "Fil does not yet support tracing in subprocesses, "
            "so starting the parent process with Fil is not sufficient for "
            "Fil APIs to work in child processes."
        )


check_if_fil_preloaded()

try:
    if sys.platform == "linux":
        # Linux only, and somehow loading library breaks stuff.
        preload = PyDLL(None)
    else:
        # macOS.
        preload = PyDLL(library_path("_filpreload"))
    preload.fil_initialize_from_python()
except Exception as e:
    raise SystemExit(
        f"""\
Failed to preload the Fil shared library: {e}.

This can happen on Linux for unknown reasons on versions of glibc older than 2.30. Upgrading to a version of Linux from 2020 or later might solve this.

If you're on macOS or a sufficiently new Linux, you've found a bug! You can file an issue or just ask for help at:
https://github.com/pythonspeed/filprofiler/issues/new/choose

Full trackback:

{traceback.format_exc()}
"""
    )


def start_tracing(output_path: Union[str, Path]):
    """Start tracing allocations."""
    preload.fil_reset(str(output_path).encode("utf-8"))
    preload.fil_start_tracking()
    threading.setprofile(_start_thread_trace)
    preload.register_fil_tracer()


def _start_thread_trace(frame, event, arg):
    """Trace function that can be passed to sys.settrace.

    All this does is register the underlying C trace function, using the
    mechanism described in
    https://github.com/nedbat/coveragepy/blob/master/coverage/ctracer/tracer.c's
    CTracer_call.
    """
    if event == "call":
        preload.register_fil_tracer()
    return _start_thread_trace


def stop_tracing(output_path: str) -> str:
    """Finish tracing allocations, and dump to disk.

    Returns path to the index HTML page of the report.
    """
    sys.setprofile(None)
    threading.setprofile(None)
    preload.fil_stop_tracking()
    result = create_report(output_path)
    # Clear allocations; we don't need them anymore, and they're just wasting
    # memory:
    preload.fil_reset("/tmp")
    return result


def create_report(output_path: Union[str, Path]) -> str:
    preload.fil_dump_peak_to_flamegraph(str(output_path).encode("utf-8"))
    now = datetime.now()
    return render_report(output_path, now)


def trace_until_exit(function, args, kwargs, output_path: str, open_browser: bool):
    """
    Given function, run it under the tracer until the program exits.
    """

    def shutdown():
        if os.environ.get("FIL_NO_REPORT"):
            print(
                "=fil-profile= FIL_NO_REPORT env variable is set, skipping report.",
                file=sys.stderr,
            )
            return
        index_path = stop_tracing(os.path.join(output_path, timestamp_now()))
        print("=fil-profile= Wrote HTML report to " + index_path, file=sys.stderr)
        if open_browser:
            try:
                webbrowser.open("file://" + os.path.abspath(index_path))
            except webbrowser.Error as e:
                print(
                    f"=fil-profile= Failed to open report in browser ({e})",
                    file=sys.stderr,
                )

    # Use atexit rather than try/finally so threads that live beyond main
    # thread also get profiled:
    atexit.register(shutdown)
    with disable_thread_pools():
        start_tracing(os.path.join(output_path, timestamp_now()))
        function(*args, **kwargs)


@contextmanager
def disable_thread_pools():
    """
    Context manager that tries to disable thread pools in as many libraries as
    possible.
    """
    try:
        from numexpr import set_num_threads as numexpr_set_num_threads
    except ImportError:

        def numexpr_set_num_threads(i):
            return 1

    try:
        from blosc import set_nthreads as blosc_set_nthreads
    except ImportError:

        def blosc_set_nthreads(i):
            return 1

    import threadpoolctl

    numexpr_threads = numexpr_set_num_threads(1)
    blosc_threads = blosc_set_nthreads(1)
    with threadpoolctl.threadpool_limits({"blas": 1, "openmp": 1}):
        try:
            yield
        finally:
            numexpr_set_num_threads(numexpr_threads)
            blosc_set_nthreads(blosc_threads)
