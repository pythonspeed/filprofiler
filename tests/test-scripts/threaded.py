from time import sleep
import threading

import numpy

def h(i):
    return numpy.ones((1024, 1024, i), dtype=numpy.uint8)

def child1():
    return h(30)

def thread1():
    data = h(20)
    sleep(0.1)
    data2 = child1()
    sleep(0.1)

threading.Thread(target=thread1).start()

def main():
    data = h(50)
    sleep(0.5)

main()
