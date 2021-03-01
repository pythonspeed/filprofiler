"""The Fil memory profiler."""

__all__ = ["__version__"]

# If we're running with Fil preloaded, after forks make sure Fil is no longer
# enabled, since we don't yet support child processes. This is also done in C
# code; doing it only in Python or only C doesn't seem to work.
import sys
import os

if sys.version_info[:2] > (3, 6):
    # register_at_fork only works in Python 3.6 or later.
    if os.getenv("__FIL_STATUS") in ("api", "program"):

        def unset(_os=os):
            _os.environ["__FIL_STATUS"] = "subprocess"

        os.register_at_fork(after_in_child=unset)
        del unset

# Fallback mechanism for detecting forks, for Python 3.6 or if someone isn't
# doing fork()-without-exec() right (i.e. not calling the C API postfork()):
_original_pid = os.getpid()

del sys, os

try:
    from ._version import version as __version__
except ImportError:
    # package is not installed
    try:
        from importlib.metadata import version, PackageNotFoundError

        try:
            __version__ = version(__name__)
        except PackageNotFoundError:
            __version__ = "unknown"
    except ImportError:
        # Python 3.6 doesn't have importlib.metadata:
        __version__ = "unknown"


def load_ipython_extension(ipython):
    """Load our IPython magic."""
    from IPython.core.error import UsageError
    import os

    if os.environ.get("__FIL_STATUS") != "api":
        raise UsageError(
            "In order to use Fil, you need to run your notebook with the Fil kernel.\n\n"
            "You can change the kernel via the 'Change Kernel' option at the bottom of "
            "the Kernel menu in Jupyter."
        )
    from ._ipython import FilMagics

    ipython.register_magics(FilMagics)
