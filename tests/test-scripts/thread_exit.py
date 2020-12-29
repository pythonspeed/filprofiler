"""Do a malloc() on thread exit."""

import ctypes
import os
from threading import Thread

C_CODE = ctypes.CDLL(
    os.path.join(os.path.dirname(__file__), "malloc_on_thread_exit.so")
)


def run():
    C_CODE.malloc_on_thread_exit()


t = Thread(target=run)
t.start()
t.join()
