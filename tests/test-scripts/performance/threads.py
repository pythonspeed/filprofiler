"""
Run some computation in threads.
"""

import time
import threading
import os
import ctypes


def calc():
    sum(range(100000))


def thread1():
    start = time.time()
    while time.time() < start + 1:
        calc()


def thread2():
    time.sleep(0.5)


t1 = threading.Thread(target=thread1)
t1.start()
threading.Thread(target=thread2).start()
# C thread:
thread = ctypes.CDLL(os.path.join(os.path.dirname(__file__), "thread.so"))
thread.sleep_in_thread()

t1.join()
