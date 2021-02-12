"""The Fil memory profiler."""

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

    if os.environ.get("__FIL_PYTHON") != "1":
        raise UsageError(
            "In order to use Fil, you need to run your notebook with the Fil kernel.\n\n"
            "You can change the kernel via the 'Change Kernel' option at the bottom of "
            "the Kernel menu in Jupyter."
        )
    from ._ipython import FilMagics

    ipython.register_magics(FilMagics)
