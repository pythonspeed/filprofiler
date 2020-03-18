import os
import sys
from pymalloc import pymalloc as malloc, pyfree as free

sys.path.append(os.path.dirname(__file__))


class Memory:
    def __init__(self, size):
        self.addr = malloc(size * 1024 * 1024)

    def __del__(self):
        free(self.addr)


def allocate(i):
    return Memory(i)


def g():
    return allocate(50)


def main():
    x = allocate(20)
    del x
    g()


main()
