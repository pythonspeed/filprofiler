"""Run code in thread after main is done."""

import time
import threading


def sleepy():
    time.sleep(0.5)


threading.Thread(target=sleepy).start()
time.sleep(0.25)
