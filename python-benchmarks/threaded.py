from time import sleep
import threading

import numpy

def h(i):
    return numpy.ones((1024, 1024, 20), dtype=numpy.uint8)

def child1():
    return h(30)

def thread1():
    data = h(20)
    sleep(1)
    data2 = child1()
    sleep(1)

def child2():
    return h(30)

def thread2():
    data = h(50)
    sleep(1)
    data2 = child2()
    sleep(1)

threading.Thread(target=thread1).start()
threading.Thread(target=thread2).start()

def main():
    data = h(50)
    sleep(5)

main()
