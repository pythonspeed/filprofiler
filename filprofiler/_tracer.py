"""Trace code, so that libpymemprofile_api know's where we are."""

import atexit
from ctypes import PyDLL
from datetime import datetime
import os
import sys
import threading
import webbrowser
from contextlib import contextmanager

from ._utils import timestamp_now, library_path
from ._report import render_report

if os.environ.get("FIL_BENCHMARK"):
    # We're using Valgrind + executable:
    preload = PyDLL(None)
else:
    # We're using preloaded library:
    preload = PyDLL(library_path("_filpreload"))
preload.fil_initialize_from_python()


def start_tracing(output_path: str):
    """Start tracing allocations."""
    path = os.path.join(output_path, timestamp_now()).encode("utf-8")
    preload.fil_reset(path)
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


def create_report(output_path: str) -> str:
    now = datetime.now()
    output_path = os.path.join(output_path, now.isoformat(timespec="milliseconds"))
    preload.fil_dump_peak_to_flamegraph(output_path.encode("utf-8"))
    return render_report(output_path, now)


def trace_until_exit(code, globals_, output_path: str):
    """
    Given code (Python or code object), run it under the tracer until the
    program exits.
    """

    def shutdown():
        if os.environ.get("FIL_NO_REPORT"):
            print(
                "=fil-profile= FIL_NO_REPORT env variable is set, skipping report.",
                file=sys.stderr,
            )
            return
        index_path = stop_tracing(output_path)
        print("=fil-profile= Wrote HTML report to " + index_path, file=sys.stderr)
        try:
            webbrowser.open("file://" + os.path.abspath(index_path))
        except webbrowser.Error:
            print(
                "=fil-profile= Failed to open browser. You can find the new run at:",
                file=sys.stderr,
            )
            print("=fil-profile= " + index_path, file=sys.stderr)

    # Use atexit rather than try/finally so threads that live beyond main
    # thread also get profiled:
    atexit.register(shutdown)
    start_tracing(output_path)
    with disable_thread_pools():
        exec(code, globals_, None)


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
