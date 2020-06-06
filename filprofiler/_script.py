"""
Command-line tools. Because of LD_PRELOAD, it's actually a two stage setup:

1. Sets ups the necessary environment variables, and then execve() stage 2.
2. Run the actual profiler CLI script.
"""

import sys
from os import environ, execv, getpid, makedirs
from os.path import abspath, dirname, join, exists
from argparse import ArgumentParser, RawDescriptionHelpFormatter, REMAINDER
import runpy
import signal

from ._utils import library_path
from . import __version__, __file__


LICENSE = """\
Copyright 2020 Hyphenated Enterprises LLC

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this program except in compliance with the License.
You may obtain a copy of the License at

     http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.


Uses additional open source libraries with the following licenses.
"""

HELP = """\
If you have a program that you usually run like this:

  $ python yourprogram.py --the-arg=x

Run it like this:

  $ fil-profile run yourprogram.py --the-arg=x

If you have a program that you usually run like this:

  $ python -m yourpackage --your-arg=2

Run it like this:

  $ fil-profile run -m yourpackage --your-arg=2

For more info visit https://pythonspeed.com/products/filmemoryprofiler/
"""


def stage_1():
    """Setup environment variables, re-execute this script."""
    # Load the library:
    environ["LD_PRELOAD"] = library_path("_filpreload")
    # Tracebacks when Rust crashes:
    environ["RUST_BACKTRACE"] = "1"
    # Route all allocations from Python through malloc() directly:
    environ["PYTHONMALLOC"] = "malloc"
    # Disable multi-threaded backends in various scientific computing libraries
    # (Zarr uses Blosc, NumPy uses BLAS):
    environ["BLOSC_NTHREADS"] = "1"
    environ["OMP_NUM_THREADS"] = "1"
    environ["OPENBLAS_NUM_THREADS"] = "1"
    environ["MKL_NUM_THREADS"] = "1"
    environ["VECLIB_MAXIMUM_THREADS"] = "1"
    environ["NUMEXPR_NUM_THREADS"] = "1"

    execv(
        sys.executable, [sys.executable, "-m", "filprofiler._script"] + sys.argv[1:],
    )


PARSER = ArgumentParser(
    usage="fil-profile [-o output-path] run [-m module | /path/to/script.py ] [arg] ...",
    epilog=HELP,
    formatter_class=RawDescriptionHelpFormatter,
    allow_abbrev=False,
)
PARSER.add_argument("--version", action="version", version=__version__)
PARSER.add_argument(
    "--license", action="store_true", default=False, help="Print licensing information",
)
PARSER.add_argument(
    "-o",
    dest="output_path",
    action="store",
    default="fil-result",
    help="Directory where the profiling results written",
)
subparsers = PARSER.add_subparsers(help="sub-command help")
parser_run = subparsers.add_parser(
    "run", help="Run a Python script or package", prefix_chars=[""], add_help=False,
)
# parser_run.add_argument(
#     "-m",
#     dest="module",
#     action="store",
#     help="Profile a module, equivalent to running with 'python -m <module>'",
#     default="",
# )
parser_run.add_argument("rest", nargs=REMAINDER)
del subparsers, parser_run


def stage_2():
    """Main CLI interface. Presumes LD_PRELOAD etc. has been set by stage_1()."""
    arguments = PARSER.parse_args()
    if arguments.license:
        print(LICENSE)
        with open(join(dirname(__file__), "licenses.txt")) as f:
            for line in f:
                print(line, end="")
        sys.exit(0)

    if arguments.rest[0] == "-m":
        # Not quite the same as what python -m does, but pretty close:
        if len(arguments.rest) == 1:
            PARSER.print_help()
            sys.exit(2)
        module = arguments.rest[1]
        sys.argv = [module] + arguments.rest[2:]
        code = "run_module(module_name, run_name='__main__')"
        globals_ = {"run_module": runpy.run_module, "module_name": module}
    else:
        sys.argv = rest = arguments.rest
        if len(rest) == 0:
            PARSER.print_help()
            sys.exit(2)
        script = rest[0]
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

    # Only import here since we don't want the parent process accessing any of
    # the _filpread.so code.
    from ._tracer import trace, create_report

    signal.signal(signal.SIGUSR2, lambda *args: create_report(arguments.output_path))
    print(
        "=fil-profile= Memory usage will be written out at exit, and opened automatically in a browser.\n"
        "=fil-profile= You can also run the following command while the program is still running to write out peak memory usage up to that point: "
        "kill -s SIGUSR2 {}".format(getpid()),
        file=sys.stderr,
    )
    if not exists(arguments.output_path):
        makedirs(arguments.output_path)
    trace(code, globals_, arguments.output_path)


if __name__ == "__main__":
    stage_2()
