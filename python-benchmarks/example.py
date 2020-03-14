import time
from os import environ
environ["BLOSC_NTHREADS"] = "1"
environ["OMP_NUM_THREADS"] = "1"
environ["OPENBLAS_NUM_THREADS"] = "1"
environ["MKL_NUM_THREADS"] = "1"
environ["VECLIB_MAXIMUM_THREADS"] = "1"
environ["NUMEXPR_NUM_THREADS"] = "1"
import numpy


def should_have_no_effect():
    h(3)


def g():
    h(1)
    h(2)
    h(1)


def return_some_data_that_isnt_freed():
    return numpy.ones((1024, 1024, 2), dtype=numpy.uint8)


def h(i):
    s = numpy.ones((1024, 1024, i), dtype=numpy.uint8)
    del s


def calc():
    arr = numpy.random.random((4096, 4096))
    arr2 = numpy.dot(arr, arr)
    return arr2


def demo():
    print("DEMO TIME!")
    start = time.time()
    g()
    should_have_no_effect()
    x = return_some_data_that_isnt_freed()
    arr = calc()
    h(5)
    print("DONE!", time.time() - start)


if __name__ == "__main__":
    demo()
