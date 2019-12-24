import ctypes
import sys
import inspect
import atexit

pymemprofile = ctypes.CDLL("target/debug/libpymemprofile.so", ctypes.RTLD_GLOBAL)

def _tracer(frame, event, arg):
    if event == "call":
        info = inspect.getframeinfo(frame)
        name = f"{info.filename}:{info.function}"
        pymemprofile.pymemprofile_start_call(name.encode("utf-8"))
    elif event == "return":
        pymemprofile.pymemprofile_finish_call()


def start_tracing():
    sys.settrace(_tracer)
    atexit.register(pymemprofile.pymemprofile_dump_functions_to_flamegraph_svg, b"/tmp/out.svg")


def g():
    h()
    h()
    h()

def h():
    s = "s" * (1024 * 1024)
    del s

def demo():
    g()
    h()

if __name__ == '__main__':
    start_tracing()
    demo()
    print("Goodbye!")
