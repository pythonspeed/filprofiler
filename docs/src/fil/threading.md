# Threading in NumPy (BLAS), Zarr, numexpr

In general, Fil will track allocations in threads correctly.

First, if you start a thread via Python, running Python code, that thread will get its own callstack for tracking who is responsible for a memory allocation.

Second, if you start a C thread, the calling Python code is considered responsible for any memory allocations in that thread.

This works fine... except for thread pools.
If you start a pool of threads that are not Python threads, the Python code that created those threads will be responsible for all allocations created during the thread pool's lifetime.
Fil therefore disables thread pools for [a number of commonly-used libraries](threadpool-disabled.md).
