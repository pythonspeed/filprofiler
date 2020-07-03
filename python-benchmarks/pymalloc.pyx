from libc.stdlib cimport malloc, free, realloc
from libc.stdint cimport uint64_t

cdef extern from "stdlib.h":
    void* aligned_alloc(size_t alignment, size_t size)

def pymalloc(size):
    return <uint64_t>malloc(size)

def pyfree(address: uint64_t):
    free(<void*>address)

def pyrealloc(address: uint64_t, size: uint64_t):
    return <uint64_t>realloc(<void*>address, size)

# aligned_alloc() isn't available on all macOS if you're doing C++ code. But it
# is available in C code, so we do it here.
def pyaligned_alloc():
    return <uint64_t>aligned_alloc(64, 1024 * 1024 * 90)
