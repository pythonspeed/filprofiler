"""Trace code, so that libpymemprofile_api.so know's where we are."""

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
    # Modeled on
    # https://github.com/nedbat/coveragepy/blob/master/coverage/ctracer/tracer.c's
    # CTracer_call.
    if event == "call":
        _profiler.start_tracing()
    return _start_thread_trace


def stop_tracing(svg_output_path: str):
    sys.settrace(None)
    path = svg_output_path.encode("utf-8")
    preload.fil_dump_peak_to_flamegraph(path)
    with open(path) as f:
        data = f.read().replace(
            "SUBTITLE-HERE",
            """Made with the Fil memory profiler. <a href="https://pythonspeed.com/products/filmemoryprofiler/" style="text-decoration: underline;" target="_parent">Try it on your code!</a>""",
        )
    with open(path, "w") as f:
        f.write(data)


def trace(code, globals_, svg_output_path: str):
    start_tracing()
    try:
        exec(code, globals_, None)
    finally:
        stop_tracing(svg_output_path)
