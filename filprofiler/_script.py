"""
Command-line tools. Because of LD_PRELOAD, it's actually a two stage setup:

1. Sets ups the necessary environment variables, and then execve() stage 2.
2. Run the actual profiler CLI script.
"""

import sys
from os import environ, execv
from os.path import abspath, dirname, join

from ._utils import library_path
from ._tracer import start_tracing, stop_tracing


def stage_1():
    """Setup environment variables, re-execute this script."""
    environ["RUST_BACKTRACE"] = "1"
    environ["PYTHONMALLOC"] = "malloc"
    # TODO dylib on Macs.
    environ["LD_PRELOAD"] = library_path("_filpreload")
    execv(sys.executable, [sys.argv[0], "-m", "filprofiler._script"] + sys.argv[1:])


def test():
    s = "aaaaaaaaaaaaaadfdsfsd352352"


def stage_2():
    """Main CLI interface. Presumes LD_PRELOAD etc. has been set by stage_1()."""
    sys.argv = args = sys.argv[1:]
    script = args[0]
    # Make directory where script is importable:
    sys.path.insert(0, dirname(abspath(script)))
    with open(script, "rb") as script_file:
        code = compile(script_file.read(), script, "exec")
    globals = {
        "__file__": script,
        "__name__": "__main__",
        "__package__": None,
        "__cached__": None,
    }
    start_tracing()
    try:
        print("STARTED")
        test()  # exec(code, globals, None)
    finally:
        print("DONE")
        stop_tracing()


if __name__ == "__main__":
    stage_2()
