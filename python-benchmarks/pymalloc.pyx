# distutils: language = c++

from libc.stdlib cimport malloc, free, realloc
from libc.stdint cimport uint64_t

cdef extern from "cpp.hpp":
    void* cppnew()

cdef extern from "stdlib.h":
    void* aligned_alloc(size_t alignment, size_t size)

def pymalloc(size):
    return <uint64_t>malloc(size)

def pyfree(address: uint64_t):
    free(<void*>address)

def pyrealloc(address: uint64_t, size: uint64_t):
    return <uint64_t>realloc(<void*>address, size)

def pycppnew():
    cppnew()

def pyaligned_alloc():
    aligned_alloc(64, 1024 * 1024 * 90)
