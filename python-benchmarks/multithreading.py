import time
import sys

from threading import Thread
from pymalloc import lots_of_allocs


def main():
    start = time.time()
    threads = [Thread(target=lots_of_allocs) for i in range(int(sys.argv[1]))]
    for t in threads:
        t.start()
    for t in threads:
        t.join()
    print("Elapsed:", time.time() - start)


if __name__ == "__main__":
    main()
