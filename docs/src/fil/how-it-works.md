# How Fil works

Fil uses the `LD_PRELOAD`/`DYLD_INSERT_LIBRARIES` mechanism to preload a shared library at process startup.
This shared library intercepts all the low-level C memory allocations and deallocation API calls, and keeps track of the corresponding allocation.

For example, instead of a [`malloc()`](https://man7.org/linux/man-pages/man3/free.3.html) memory allocation going directly to your operating system, Fil will intercept it, keep note of the allocation, and then call the underlying implementation of `malloc()`.

At the same time, the Python tracing infrastructure (what `cProfile` and `coverage.py` use) is used to figure out which Python callstack/backtrace is responsible for each allocation.

While every single allocation is tracked, for performance reasons only the largest allocations are reported, with a minimum of 99% of allocated memory reported.
The remaining <1% is highly unlikely to be relevant when trying to reduce usage; it's effectively noise.

On Linux, Fil replaces the standard glibc allocator with [`jemalloc`](http://jemalloc.net/), though this is an implementation detail that may change in the future.
