from libc.stdlib cimport malloc, free
from libc.stdint cimport uint64_t

def pymalloc(size):
    return <uint64_t>malloc(size)

def pyfree(address: uint64_t):
    free(<void*>address)
