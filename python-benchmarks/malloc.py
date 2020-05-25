import os
import sys
from argparse import ArgumentParser
from pymalloc import pymalloc as malloc, pyfree as free

sys.path.append(os.path.dirname(__file__))

# If malloc() is captured, so is free() etc, so less important to test those.
def main():
    parser = ArgumentParser()
    parser.add_argument("--size", action="store")
    args = parser.parse_args()
    malloc(int(args.size) * 1024 * 1024)


main()
