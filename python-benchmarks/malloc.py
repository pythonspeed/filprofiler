import os
import sys
from argparse import ArgumentParser
from pymalloc import (
    pymalloc,
    pyrealloc,
    pyaligned_alloc,
    pyallocation_api,
    pyposix_memalign,
)
import ctypes

CPP = ctypes.PyDLL(os.path.join(os.path.dirname(__file__), "cpp.so"))
sys.path.append(os.path.dirname(__file__))

MB = 1024 * 1024


def main():
    parser = ArgumentParser()
    parser.add_argument("--size", action="store")
    size = int(parser.parse_args().size)
    CPP.cppnew()
    pyaligned_alloc()
    pyallocation_api()
    pyposix_memalign()
    result = pymalloc(size * MB)
    result = pyrealloc(result, (size + 10) * MB)  # <-- peak
    result = pyrealloc(result, (size - 5) * MB)


main()
