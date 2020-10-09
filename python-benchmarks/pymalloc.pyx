from libc.stdlib cimport malloc, free, realloc
from libc.stdint cimport uint64_t

cdef extern from "stdlib.h":
    void* aligned_alloc(size_t alignment, size_t size)
    int posix_memalign(void **memptr, size_t alignment, size_t size)

cdef extern from "Python.h":
    void* PyMem_Malloc(size_t n)
    void* PyObject_Malloc(size_t n)
    void* PyMem_RawMalloc(size_t n)

def pymalloc(size):
    return <uint64_t>malloc(size)

def pyfree(address: uint64_t):
    free(<void*>address)

def pyrealloc(address: uint64_t, size: uint64_t):
    return <uint64_t>realloc(<void*>address, size)

# aligned_alloc() isn't available on all macOS if you're doing C++ code. But it
# is sometimes available in C code, so we do it here.
def pyaligned_alloc():
    return <uint64_t>aligned_alloc(64, 1024 * 1024 * 90)

def pyallocation_api():
    return [
        <uint64_t>PyMem_Malloc(1024 * 1024 * 10),
        <uint64_t>PyObject_Malloc(1024 * 1024 * 10),
        <uint64_t>PyMem_RawMalloc(1024 * 1024 * 10),
    ]

def pyposix_memalign():
    cdef void *result;
    return posix_memalign(&result, 64, 1024 * 1024 * 15)


def lots_of_allocs():
    cdef uint64_t i
    with nogil:
        for i in range(10000000):
            p = <uint64_t*>malloc(16)
            p[0] = 1
            free(p)
    # Garbage, but without this the compiler optimizes the whole loop out.
    return p[0]
