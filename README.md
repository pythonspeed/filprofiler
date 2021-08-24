# The Fil memory profiler for Python

Your code reads some data, processes it, and uses too much memory.
In order to reduce memory usage, you need to figure out:

1. Where peak memory usage is, also known as the high-water mark.
2. What code was responsible for allocating the memory that was present at that peak moment.

That's exactly what Fil will help you find.
Fil an open source memory profiler designed for data processing applications written in Python, and includes native support for Jupyter.

At the moment it only runs on Linux and macOS, and while it supports threading, it does not yet support multiprocessing or multiple processes in general.

> "Within minutes of using your tool, I was able to identify a major memory bottleneck that I never would have thought existed.  The ability to track memory allocated via the Python interface and also C allocation is awesome, especially for my NumPy / Pandas programs."
> 
> —Derrick Kondo

For more information, including an example of the output, see https://pythonspeed.com/products/filmemoryprofiler/

* [Fil vs. other Python memory tools](#other-tools)
* [Installation](#installation)
* [Using Fil](#using-fil)
    * [Profiling in Jupyter](#peak-jupyter)
    * [Profiling complete Python programs](#peak-python)
    * [API for profiling specific Python functions](#code)
    * [Debugging out-of-memory crashes in your code](#oom)
* [Reducing memory usage in your code](#reducing-memory-usage)
* [How Fil works](#how-fil-works)
    * [Fil and threading, with notes on NumPy and Zarr](#threading)
    * [What Fil tracks](#what-fil-tracks)

## Using Fil


### Fil and threading, with notes on NumPy and Zarr {#threading}

In general, Fil will track allocations in threads correctly.

First, if you start a thread via Python, running Python code, that thread will get its own callstack for tracking who is responsible for a memory allocation.

Second, if you start a C thread, the calling Python code is considered responsible for any memory allocations in that thread.
This works fine... except for thread pools.
If you start a pool of threads that are not Python threads, the Python code that created those threads will be responsible for all allocations created during the thread pool's lifetime.

Therefore, in order to ensure correct memory tracking, Fil disables thread pools in  BLAS (used by NumPy), BLOSC (used e.g. by Zarr), OpenMP, and `numexpr`.
They are all set to use 1 thread, so calls should run in the calling Python thread and everything should be tracked correctly.

This has some costs:

1. This can reduce performance in some cases, since you're doing computation with one CPU instead of many.
2. Insofar as these libraries allocate memory proportional to number of threads, the measured memory usage might be wrong.

Fil does this for the whole program when using `fil-profile run`.
When using the Jupyter kernel, anything run with the `%%filprofile` magic will have thread pools disabled, but other code should run normally.

### What Fil tracks

Fil will track memory allocated by:

* Normal Python code.
* C code using `malloc()`/`calloc()`/`realloc()`/`posix_memalign()`.
* C++ code using `new` (including via `aligned_alloc()`).
* Anonymous `mmap()`s.
* Fortran 90 explicitly allocated memory (tested with gcc's `gfortran`).

Still not supported, but planned:

* `mremap()` (resizing of `mmap()`).
* File-backed `mmap()`.
  The semantics are somewhat different than normal allocations or anonymous `mmap()`, since the OS can swap it in or out from disk transparently, so supporting this will involve a different kind of resource usage and reporting.
* Other forms of shared memory, need to investigate if any of them allow sufficient allocation.
* Anonymous `mmap()`s created via `/dev/zero` (not common, since it's not cross-platform, e.g. macOS doesn't support this).
* `memfd_create()`, a Linux-only mechanism for creating in-memory files.
* Possibly `memalign`, `valloc()`, `pvalloc()`, `reallocarray()`. These are all rarely used, as far as I can tell.

## License

Copyright 2020 Hyphenated Enterprises LLC

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

     http://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
