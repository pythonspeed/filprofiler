"""Make sure Fil notices `mmap()`."""
from mmap import mmap

x = mmap(-1, 1024 * 1024 * 50)
del x
y = mmap(-1, 1024 * 1024 * 60)
