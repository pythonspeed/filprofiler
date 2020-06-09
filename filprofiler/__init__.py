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
