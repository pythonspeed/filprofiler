import ctypes
import sys
import inspect
import atexit
import numpy

pymemprofile = ctypes.CDLL("target/debug/libpymemprofile_api.so")

def _tracer(frame, event, arg):
    if event == "call":
        info = inspect.getframeinfo(frame)
        name = f"{info.filename}:{info.function}"
        pymemprofile.pymemprofile_start_call(name.encode("utf-8"))
    elif event == "return":
        pymemprofile.pymemprofile_finish_call()
    return _tracer

def start_tracing():
    sys.settrace(_tracer)
    atexit.register(pymemprofile.pymemprofile_dump_functions_to_flamegraph_svg, b"/tmp/out.svg")


def g():
    h(1)
    h(2)
    h(1)

def h(i):
    s = numpy.ones((1024, 1024, i), dtype=numpy.uint8)
    del s

def demo():
    g()
    h(5)

if __name__ == '__main__':
    start_tracing()
    demo()
