# Fil: A memory profiler for Python

Your Python code reads some data, processes it, and uses too much memory; maybe it even dies due to an out-of-memory error.
In order to reduce memory usage, you first need to figure out:

1. Where peak memory usage is, also known as the high-water mark.
2. What code was responsible for allocating the memory that was present at that peak moment.

That's exactly what Fil will help you find.
Fil an open source memory profiler designed for data processing applications written in Python, and includes native support for Jupyter.

Fil comes in two editions:

* An open source version, designed for offline profiling.
  It tracks all allocations, and runs on Linux and macOS, but has enough of a performance impact that you won't want to use it on production workloads.
* A commercial production version, that is fast enough to run on _all_ your production data processing batch jobs.
  As a trade-off, it only runs on Linux, and samples memory allocations: unlike the more generally useful open source edition, the production version is optimized for data-intensive programs that allocate large amounts of memory.
