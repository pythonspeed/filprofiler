"""Utilities."""

from importlib.util import find_spec
from datetime import datetime
import ctypes
from typing import Tuple


def library_path(name):
    """Return the path of a shared library."""
    return find_spec("filprofiler." + name).origin


def timestamp_now() -> str:
    """Return current time as a string."""
    now = datetime.now()
    return now.isoformat(timespec="milliseconds")


def glibc_version() -> Tuple[int, int]:
    """Get the version of glibc."""
    libc = ctypes.CDLL("libc.so.6")
    get_libc_version = libc.gnu_get_libc_version
    get_libc_version.restype = ctypes.c_char_p
    return tuple(map(int, get_libc_version().split(b".")[:2]))
