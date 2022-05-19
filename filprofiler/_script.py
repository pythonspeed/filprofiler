"""
Command-line tools. Because of LD_PRELOAD, it's actually a two stage setup:

1. Sets ups the necessary environment variables, and then execve() stage 2.
2. Run the actual profiler CLI script.
"""

import json
import sys
import os
from os import environ, execve, getpid, makedirs
from os.path import abspath, dirname, join, exists
from argparse import ArgumentParser, RawDescriptionHelpFormatter, REMAINDER
from typing import List
import runpy
import signal
from shutil import which
from ._utils import library_path, glibc_version, timestamp_now
from ._cachegrind import benchmark
from . import __version__, __file__


LICENSE = """\
Copyright 2021 Hyphenated Enterprises LLC

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

You can also run the profiler this way:

  $ python -m filprofiler run yourprogram.py

You can read the documentation at https://pythonspeed.com/fil/docs/
"""


PARSER = ArgumentParser(
    usage="fil-profile [-o output-path] run [-m module | /path/to/script.py ] [arg] ...",
    epilog=HELP,
    formatter_class=RawDescriptionHelpFormatter,
    allow_abbrev=False,
)
PARSER.add_argument("--version", action="version", version=__version__)
PARSER.add_argument(
    "--license",
    action="store_true",
    default=False,
    help="Print licensing information",
)
PARSER.add_argument(
    "-o",
    dest="output_path",
    action="store",
    default="fil-result",
    help="Directory where the profiling results written",
)
PARSER.add_argument(
    "--disable-oom-detection",
    action="store_true",
    default=False,
    help="Disable the heuristic that tries to catch out-of-memory situations before they occur",
)
PARSER.add_argument(
    "--no-browser",
    action="store_true",
    help="Don't try to open the profiling report in a browser.",
)
subparsers = PARSER.add_subparsers(help="sub-command help")
parser_run = subparsers.add_parser(
    "run",
    help="Run a Python script or package with Fil enabled",
    prefix_chars=[""],
    add_help=False,
)
parser_python = subparsers.add_parser(
    "python",
    help="Run a Python script or package with Fil initially disabled",
    prefix_chars=[""],
    add_help=False,
)
parser_run.set_defaults(command="run")
parser_run.add_argument("rest", nargs=REMAINDER)
parser_python.set_defaults(command="python")
parser_python.add_argument("rest", nargs=REMAINDER)
del subparsers, parser_run, parser_python

# Can't figure out if this is a standard path _everywhere_, but it definitely
# exists on Ubuntu 18.04 and 20.04, Debian Buster, CentOS 8, and Arch.
LD_LINUX = "/lib64/ld-linux-x86-64.so.2"


def stage_1_benchmark(args: List[str]):
    """Run the script in benchmarking mode."""
    destination = environ["FIL_BENCHMARK"]
    # Set fixed hash, in order to get repeatable results:
    environ["PYTHONHASHSEED"] = "12345"

    # We run the script twice, once with just normal Python, once with Fil,
    # and report the difference. That way we're measuring overhead, and not
    # counting time that's just Python doing its normal thing.
    # 1. Run with just Python:
    if sys.argv[1] == "python":
        pyargs = sys.argv[2:]
    else:
        arguments = PARSER.parse_args()
        pyargs = arguments.rest
    python_result = benchmark([sys.executable] + pyargs)
    # 2. Run using Valgrind. Valgrind has its own LD_PRELOAD which has
    # issues with our own, so we add our extra LD_PRELOAD by using
    # command-line based non-execve()ing /lib/ld.so's preload support,
    # without having Valgrind trace forks.
    fil_result = benchmark(
        [LD_LINUX, "--preload", library_path("_filpreload"), which("python")] + args
    )
    # 3. Store the difference.
    result = {
        k: "{} ({}%)".format(
            (fil_result[k] - python_result[k]), fil_result[k] * 100 / python_result[k]
        )
        for k in fil_result
    }
    with open(destination, "w+") as f:
        json.dump(result, f, sort_keys=True, indent=4)
        f.flush()
        f.seek(0, 0)
        print("Wrote performance to {}:".format(destination))
        print(f.read())


def stage_1():
    """Setup environment variables, re-execute this script."""
    if len(sys.argv) == 1:
        PARSER.print_help()
        sys.exit(0)

    arguments = PARSER.parse_args()
    if arguments.disable_oom_detection:
        # See filpreload/src/lib.rs:
        environ["__FIL_DISABLE_OOM_DETECTION"] = "1"

    # Initial status:
    environ["__FIL_STATUS"] = "launcher"
    # Tracebacks when Rust crashes:
    environ["RUST_BACKTRACE"] = "1"
    # Route all allocations from Python through malloc() directly:
    environ["PYTHONMALLOC"] = "malloc"
    # Tell jemalloc code (if used) to clean up faster:
    if environ.get("_RJEM_MALLOC_CONF") is None:
        environ[
            "_RJEM_MALLOC_CONF"
        ] = "dirty_decay_ms:100,muzzy_decay_ms:1000,abort_conf:true"

    if sys.argv[1] == "python":
        # Tells IPython layer we're setup correctly:
        environ["__FIL_STATUS"] = "api"
        # Start the normal Python interpreter, with Fil available but inactive.
        args = sys.argv[2:]
    else:
        environ["__FIL_STATUS"] = "program"
        args = ["-m", "filprofiler._script"] + sys.argv[1:]

    if environ.get("FIL_BENCHMARK"):
        stage_1_benchmark(args)
        return

    # Normal operation, via LD_PRELOAD or equivalent:
    to_preload = library_path("_filpreload")
    executable = sys.executable

    if sys.platform == "linux":
        if glibc_version() >= (2, 30) and exists(LD_LINUX):
            # Launch with ld.so, which is more robust than relying on
            # environment variables.
            executable = LD_LINUX
            args = ["--preload", to_preload, sys.executable] + args
        else:
            # Fall back to LD_PRELOAD env variable on older versions of glibc.
            environ["LD_PRELOAD"] = to_preload
    else:
        environ["DYLD_INSERT_LIBRARIES"] = to_preload

    execve(executable, [executable] + args, env=environ)


def stage_2():
    """Main CLI interface for `fil-profile run`.

    Presumes LD_PRELOAD etc. has been set by stage_1().
    """
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
        function = runpy.run_module
        func_args = (module,)
        func_kwargs = {"run_name": "__main__", "alter_sys": True}
    else:
        sys.argv = rest = arguments.rest
        if len(rest) == 0:
            PARSER.print_help()
            sys.exit(2)
        script = rest[0]
        # Make directory where script is importable:
        sys.path.insert(0, dirname(abspath(script)))
        function = runpy.run_path
        func_args = (script,)
        func_kwargs = {"run_name": "__main__"}

    # Only import here since we don't want the parent process accessing any of
    # the _filpread.so code.
    from ._tracer import trace_until_exit, create_report

    signal.signal(
        signal.SIGUSR2,
        lambda *args: create_report(
            os.path.join(arguments.output_path, timestamp_now())
        ),
    )

    msg_browser = ", and opened automatically in a browser" if not arguments.no_browser else ", and stored in {}.".format(arguments.output_path)
    msg_fil_plan = "=fil-profile= Memory usage will be written out at exit{}.\n".format(msg_browser)
    print(
        msg_fil_plan +
        "=fil-profile= You can also run the following command while the program is still running to write out peak memory usage up to that point: " +
        "kill -s SIGUSR2 {}".format(getpid()),
        file=sys.stderr,
    )
    if not exists(arguments.output_path):
        makedirs(arguments.output_path)

    trace_until_exit(
        function,
        func_args,
        func_kwargs,
        arguments.output_path,
        not arguments.no_browser,
    )


if __name__ == "__main__":
    stage_2()
