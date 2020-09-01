"""Test allocating in C in a non-Python thread."""

import ctypes
import os


def main():
    CPP = ctypes.PyDLL(os.path.join(os.path.dirname(__file__), "cpp.so"))
    CPP.allocate_in_thread()


if __name__ == "__main__":
    main()
