"""Trace code, so that libpymemprofile_api.so know's where we are."""

import atexit
import os
import sys
import threading
from ctypes import CDLL, RTLD_GLOBAL

from ._utils import library_path

# Load with RTLD_GLOBAL so _profiler.so has access to those symbols; explicit
# linking may be possible but haven't done that yet, oh well.
pymemprofile = CDLL(library_path("libpymemprofile_api"), mode=RTLD_GLOBAL)
preload = CDLL(library_path("_filpreload"), mode=RTLD_GLOBAL)
from . import _profiler


def start_tracing():
    preload.fil_reset()
    threading.settrace(_start_thread_trace)
    _profiler.start_tracing()


def _start_thread_trace(frame, event, arg):
    """Trace function that can be passed to sys.settrace.

    All this does is register the underlying C trace function, using the
    mechanism described in
    https://github.com/nedbat/coveragepy/blob/master/coverage/ctracer/tracer.c's
    CTracer_call.
    """
    if event == "call":
        _profiler.start_tracing()
    return _start_thread_trace


def stop_tracing(output_path: str):
    sys.settrace(None)
    dump_svg(output_path)


def dump_svg(output_path: str):
    path = output_path.encode("utf-8")
    preload.fil_dump_peak_to_flamegraph(path)
    for svg_path in [
        os.path.join(output_path, "peak-memory.svg"),
        os.path.join(output_path, "peak-memory-reversed.svg"),
    ]:
        with open(svg_path) as f:
            data = f.read().replace(
                "SUBTITLE-HERE",
                """Made with the Fil memory profiler. <a href="https://pythonspeed.com/products/filmemoryprofiler/" style="text-decoration: underline;" target="_parent">Try it on your code!</a>""",
            )
            with open(svg_path, "w") as f:
                f.write(data)


def trace(code, globals_, output_path: str):
    """
    Given code (Python or code object), run it under the tracer until the
    program exits.
    """
    atexit.register(stop_tracing, output_path)
    start_tracing()
    exec(code, globals_, None)
