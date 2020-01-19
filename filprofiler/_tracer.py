"""Trace code, so that libpymemprofile_api.so know's where we are."""

import inspect
import sys
from ctypes import CDLL

from ._utils import library_path
pymemprofile = CDLL(library_path("libpymemprofile_api"))


def _tracer(frame, event, arg):
    """Tracing function for sys.settrace."""
    if event == "call":
        info = inspect.getframeinfo(frame)
        name = f"{info.filename}:{info.function}"
        pymemprofile.pymemprofile_start_call(name.encode("utf-8"))
    elif event == "return":
        pymemprofile.pymemprofile_finish_call()
    return _tracer


def start_tracing():
    pymemprofile.pymemprofile_reset()
    sys.settrace(_tracer)

def stop_tracing():
    pymemprofile.pymemprofile_dump_peak_to_flamegraph(b"/tmp/out.svg")


def trace(code, globals_):
    start_tracing()
    try:
        exec(code, globals_, None)
    finally:
        stop_tracing()
