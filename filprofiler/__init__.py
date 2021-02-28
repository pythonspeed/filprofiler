"""The Fil memory profiler."""

__all__ = ["__version__"]

from os import getenv, unsetenv, register_at_fork

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

    if os.environ.get("__FIL_PYTHON") != "api":
        raise UsageError(
            "In order to use Fil, you need to run your notebook with the Fil kernel.\n\n"
            "You can change the kernel via the 'Change Kernel' option at the bottom of "
            "the Kernel menu in Jupyter."
        )
    from ._ipython import FilMagics

    ipython.register_magics(FilMagics)


# After forks, make sure Fil is no longer enabled, since we don't yet support child processes:
if getenv("__FIL_PYTHON"):
    register_at_fork(after_in_child=lambda unsetenv=unsetenv: unsetenv("__FIL_PYTHON"))
del getenv, unsetenv, register_at_fork
