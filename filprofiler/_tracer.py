"""Trace code, so that libpymemprofile_api.so know's where we are."""

import sys
from ctypes import CDLL, RTLD_GLOBAL

from ._utils import library_path

# Load with RTLD_GLOBAL so _profiler.so has access to those symbols; explicit
# linking may be possible but haven't done that yet, oh well.
pymemprofile = CDLL(library_path("libpymemprofile_api"), mode=RTLD_GLOBAL)
preload = CDLL(library_path("_filpreload"), mode=RTLD_GLOBAL)
from . import _profiler


def start_tracing():
    preload.fil_reset()
    _profiler.start_tracing()


def stop_tracing(svg_output_path: str):
    sys.settrace(None)
    path = svg_output_path.encode("utf-8")
    preload.fil_dump_peak_to_flamegraph(path)
    with open(path) as f:
        data = f.read().replace(
            "SUBTITLE-HERE",
            """Created by the Fil memory profiler. <a href="https://pythonspeed.com/products/filmemoryprofiler/" style="text-decoration: underline;">Want to try it on your code?</a>""",
        )
    with open(path, "w") as f:
        f.write(data)


def trace(code, globals_, svg_output_path: str):
    start_tracing()
    try:
        exec(code, globals_, None)
    finally:
        stop_tracing(svg_output_path)
