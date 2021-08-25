# Fil: A memory profiler for Python

Your Python code reads some data, processes it, and uses too much memory; maybe it even dies due to an out-of-memory error.
In order to reduce memory usage, you first need to figure out:

1. Where peak memory usage is, also known as the high-water mark.
2. What code was responsible for allocating the memory that was present at that peak moment.

That's exactly what Fil will help you find.
Fil an open source memory profiler designed for data processing applications written in Python, and includes native support for Jupyter.

Fil comes in two editions:

* **The open source edition:** designed for offline profiling.
  It has enough of a performance impact that you won't want to use it on production workloads, but it can profile even small amounts of memory.
* **The commercial, production version:** optimized for data-intensive programs that allocate large amounts of memory, is is fast enough to run on all your production data processing batch jobs.
