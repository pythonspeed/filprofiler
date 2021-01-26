from libc.stdlib cimport malloc, free
from libc.stdint cimport uint64_t

def lots_of_allocs():
    cdef uint64_t i, j
    cdef uint64_t *ps[1000]
    with nogil:
        for j in range(100):
            for i in range(1000):
                ps[i] = <uint64_t*>malloc(16)
                ps[i][0] = 1
            # If we just free everything, we get... noise based on previous
            # allocations. So try to smooth with increasing allocations.
            for i in range(500):
                free(ps[i])
    # Garbage, but without this the compiler might optimize the whole loop out.
    return <uint64_t>ps[0]
