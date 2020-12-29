from time import sleep
import threading

import numpy

def thread1():
    # Main allocation after main thread exits:
    sleep(0.5)
    data = numpy.ones((1024, 1024, 70), dtype=numpy.uint8)
    sleep(0.5)

threading.Thread(target=thread1).start()

def main():
    sleep(0.1)

main()
