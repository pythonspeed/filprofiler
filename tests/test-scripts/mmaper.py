"""Make sure Fil notices `mmap()`."""
import mmap
import sys


x = mmap.mmap(-1, 1024 * 1024 * 50)
del x
y = mmap.mmap(-1, 1024 * 1024 * 60)

if sys.platform == "linux":
    from ctypes import CDLL; current_process = CDLL(None)
    # Multi-line traceback line numbers differ across different versions of
    # Python, so doing this all on one line for consistency:
    z = current_process.mmap(0, 1024 * 1024 * 45, mmap.PROT_READ | mmap.PROT_WRITE, mmap.MAP_PRIVATE | mmap.MAP_ANONYMOUS, -1, 0)
    z = current_process.mmap64(0, 1024 * 1024 * 63, mmap.PROT_READ | mmap.PROT_WRITE, mmap.MAP_PRIVATE | mmap.MAP_ANONYMOUS, -1, 0)
