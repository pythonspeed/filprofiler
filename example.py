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
    pymemprofile.pymemprofile_reset()
    sys.settrace(_tracer)
    atexit.register(pymemprofile.pymemprofile_dump_peak_to_flamegraph, b"/tmp/out.svg")


import gc

def should_have_no_effect():
    h(3)

def g():
    h(1)
    h(2)
    h(1)

def return_some_data_that_isnt_freed():
    return numpy.ones((1024, 1024, 2), dtype=numpy.uint8)

def h(i):
    s = numpy.ones((1024, 1024, i), dtype=numpy.uint8)
    del s

def demo():
    g()
    should_have_no_effect()
    x = return_some_data_that_isnt_freed()
    h(5)

if __name__ == '__main__':
    start_tracing()
    demo()
