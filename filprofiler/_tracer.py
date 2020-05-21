"""Trace code, so that libpymemprofile_api know's where we are."""

import atexit
from ctypes import PyDLL
from datetime import datetime
import os
import sys
import threading
import webbrowser

from ._report import render_report

# None effectively means RTLD_NEXT, it seems.
preload = PyDLL(None)
preload.fil_initialize_from_python()


def start_tracing():
    preload.fil_reset()
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


def stop_tracing(output_path: str):
    sys.setprofile(None)
    threading.setprofile(None)
    create_report(output_path)
    preload.fil_shutting_down()


def create_report(output_path: str):
    now = datetime.now()
    output_path = os.path.join(output_path, now.isoformat(timespec="milliseconds"))
    preload.fil_dump_peak_to_flamegraph(output_path.encode("utf-8"))
    index_path = render_report(output_path, now)

    print("=fil-profile= Wrote HTML report to " + index_path, file=sys.stderr)
    try:
        webbrowser.open(index_path)
    except webbrowser.Error:
        print(
            "=fil-profile= Failed to open browser. You can find the new run at:",
            file=sys.stderr,
        )
        print("=fil-profile= " + index_path, file=sys.stderr)


def trace(code, globals_, output_path: str):
    """
    Given code (Python or code object), run it under the tracer until the
    program exits.
    """
    # Use atexit rather than try/finally so threads that live beyond main
    # thread also get profiled:
    atexit.register(stop_tracing, output_path)
    start_tracing()
    exec(code, globals_, None)
