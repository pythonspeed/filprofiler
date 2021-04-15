"""Make sure Fil notices `mmap()`."""
import mmap
import sys
from ctypes import CDLL

x = mmap.mmap(-1, 1024 * 1024 * 50)
del x
y = mmap.mmap(-1, 1024 * 1024 * 60)

if sys.platform == "linux":
    current_process = CDLL(None)
    z = current_process.mmap(
        0,
        1024 * 1024 * 45,
        mmap.PROT_READ | mmap.PROT_WRITE,
        mmap.MAP_PRIVATE | mmap.MAP_ANONYMOUS,
        -1,
        0,
    )
    a = current_process.mmap64(
        0,
        1024 * 1024 * 63,
        mmap.PROT_READ | mmap.PROT_WRITE,
        mmap.MAP_PRIVATE | mmap.MAP_ANONYMOUS,
        -1,
        0,
    )
