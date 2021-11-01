"""
Just your basic spinning in a loop for 1 second.
"""

import time

def calc():
    sum(range(100000))

start = time.time()
while time.time() < start + 1:
    calc()
