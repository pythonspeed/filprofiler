"""
Command-line tools. Because of LD_PRELOAD, it's actually a two stage setup:

1. Sets ups the necessary environment variables, and then execve() stage 2.
2. Run the actual profiler CLI script.
"""

import sys
from time import asctime
from os import environ, execv, getpid, makedirs
from os.path import abspath, dirname, join, exists
from argparse import ArgumentParser
import runpy
import signal

from ._utils import library_path
from ._tracer import trace, dump_svg
from . import __version__


def stage_1():
    """Setup environment variables, re-execute this script."""
    # Tracebacks when Rust crashes:
    environ["RUST_BACKTRACE"] = "1"
    # Route all allocations from Python through malloc() directly:
    environ["PYTHONMALLOC"] = "malloc"
    # Library setup:
    environ["LD_PRELOAD"] = library_path("_filpreload")
    environ["FIL_API_LIBRARY"] = library_path("libpymemprofile_api")
    # Disable multi-threaded backends in various scientific computing libraries
    # (Zarr uses Blosc, NumPy uses BLAS):
    environ["BLOSC_NTHREADS"] = "1"
    environ["OMP_NUM_THREADS"] = "1"
    environ["OPENBLAS_NUM_THREADS"] = "1"
    environ["MKL_NUM_THREADS"] = "1"
    environ["VECLIB_MAXIMUM_THREADS"] = "1"
    environ["NUMEXPR_NUM_THREADS"] = "1"

    execv(sys.executable, [sys.argv[0], "-m", "filprofiler._script"] + sys.argv[1:])


def stage_2():
    """Main CLI interface. Presumes LD_PRELOAD etc. has been set by stage_1()."""
    usage = "fil-profile [-o /path/to/output-dir/] [-m module | /path/to/script.py ] [arg] ..."
    parser = ArgumentParser(usage=usage)
    parser.add_argument("--version", action="version", version=__version__)
    parser.add_argument(
        "-o",
        dest="output_path",
        action="store",
        default="fil-result",
        help="Directory where the profiling results written.",
    )
    parser.add_argument(
        "-m",
        dest="module",
        action="store",
        help="Profile a module, equivalent to running with 'python -m <module>'",
        default="",
    )
    parser.add_argument("args", metavar="ARG", nargs="*")
    arguments = parser.parse_args()
    if arguments.module:
        # Not quite the same as what python -m does, but pretty close:
        sys.argv = [arguments.module] + arguments.args
        code = "run_module(module_name, run_name='__main__')"
        globals_ = {"run_module": runpy.run_module, "module_name": arguments.module}
    else:
        sys.argv = args = arguments.args
        script = args[0]
        # Make directory where script is importable:
        sys.path.insert(0, dirname(abspath(script)))
        with open(script, "rb") as script_file:
            code = compile(script_file.read(), script, "exec")
        globals_ = {
            "__file__": script,
            "__name__": "__main__",
            "__package__": None,
            "__cached__": None,
        }
    signal.signal(
        signal.SIGUSR2, lambda *args: dump_svg(join(arguments.output_path, asctime()))
    )
    print(
        "=fil-profile= Run the following command to write out peak memory usage: "
        "kill -s SIGUSR2 {}".format(getpid()),
        file=sys.stderr,
    )
    if not exists(arguments.output_path):
        makedirs(arguments.output_path)
    trace(code, globals_, arguments.output_path)


if __name__ == "__main__":
    stage_2()
