import os
import sys
from argparse import ArgumentParser
from pymalloc import pymalloc as malloc, pyfree as free, pyrealloc as realloc

sys.path.append(os.path.dirname(__file__))

MB = 1024 * 1024

# If malloc() is captured, so is free() etc, so less important to test those.
def main():
    parser = ArgumentParser()
    parser.add_argument("--size", action="store")
    args = parser.parse_args()
    size = int(args.size)
    result = malloc(size * MB)
    result = realloc(result, (size + 10) * MB)  # <-- peak
    result = realloc(result, (size - 5) * MB)


main()
