"""Some code that uses the Fil API."""

from tempfile import mkdtemp
from filprofiler.api import profile


def run_with_fil():
    def f():
        return 123

    return profile(f, mkdtemp())
