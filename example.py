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
    h()
    h()
    h()

def h():
    s = numpy.ones((1024, 1024), dtype=numpy.uint8)
    del s

def demo():
    g()
    h()

if __name__ == '__main__':
    start_tracing()
    demo()
    print("Goodbye!")
