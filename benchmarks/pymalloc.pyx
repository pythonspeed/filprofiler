from libc.stdlib cimport malloc, free
from libc.stdint cimport uint64_t

def lots_of_allocs():
    cdef uint64_t i
    with nogil:
        for i in range(10000000):
            p = <uint64_t*>malloc(16)
            p[0] = 1
            free(p)
    # Garbage, but without this the compiler optimizes the whole loop out.
    return p[0]
