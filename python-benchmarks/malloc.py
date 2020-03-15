from ctypes import CDLL, c_void_p
from ctypes.util import find_library

exe = CDLL(None)
malloc = exe.malloc
malloc.restype = c_void_p
free = exe.free
free.argtypes = (c_void_p,)


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
    y = g()


main()
