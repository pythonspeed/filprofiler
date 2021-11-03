"""Two threads, fighting over the GIL!"""

import threading
import time


def calc():
    sum(range(10000))


def go():
    start = time.time()
    while time.time() < start + 2.0:
        calc()


threading.Thread(target=go).start()
go()
