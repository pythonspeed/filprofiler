Switched to using jemalloc on Linux, which should deal better both in terms of memory usage and speed with many small allocations.
It also simplifies the code.