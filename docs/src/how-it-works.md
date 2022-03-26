# How Fil works

Fil uses the `LD_PRELOAD`/`DYLD_INSERT_LIBRARIES` mechanism to preload a shared library at process startup.
This is why Fil can't be used as regular library and needs to be started in a special way: it requires setting up the correct environment _before_ Python starts.

This shared library intercepts all [the low-level C memory allocation and deallocation API calls](what-it-tracks.md), and keeps track of the corresponding allocation.
For example, instead of a [`malloc()`](https://man7.org/linux/man-pages/man3/free.3.html) memory allocation going directly to your operating system, Fil will intercept it, keep note of the allocation, and then call the underlying implementation of `malloc()`.

At the same time, the Python tracing infrastructure (the same infrastructure used by `cProfile` and `coverage.py`) is used to figure out which Python callstack/backtrace is responsible for each allocation.
