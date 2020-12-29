import time
import sys

from threading import Thread
from pymalloc import lots_of_allocs


def main():
    start = time.time()
    num_threads = int(sys.argv[1])
    # If there's only one-thread, just run in main thread:
    if num_threads == 1:
        lots_of_allocs()
    else:
        threads = [Thread(target=lots_of_allocs) for i in range(num_threads)]
        for t in threads:
            t.start()
        for t in threads:
            t.join()
    print("Elapsed:", time.time() - start)


if __name__ == "__main__":
    main()
