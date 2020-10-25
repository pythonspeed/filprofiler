# The Fil memory profiler for Python

Your code reads some data, processes it, and uses too much memory.
In order to reduce memory usage, you need to learn what code is responsible, and specifically what code is responsible for peak memory usage.

And that's exactly what Fil will help you find.
Fil an open source memory profiler designed for data processing applications written in Python, and includes native support for Jupyter.

At the moment it only runs on Linux and macOS.

> "Within minutes of using your tool, I was able to identify a major memory bottleneck that I never would have thought existed.  The ability to track memory allocated via the Python interface and also C allocation is awesome, especially for my NumPy / Pandas programs."
> 
> —Derrick Kondo

For more information, including an example of the output, see https://pythonspeed.com/products/filmemoryprofiler/

* [Installation](#installation)
* [Using Fil](#using-fil)
    * [Measuring peak (high-water mark) memory usage in Jupyter](#peak-jupyter)
    * [Measuring peak memory usage for Python scripts](#peak-python)
    * [Debugging out-of-memory crashes in your code](#oom)
* [Reducing memory usage in your code](#reducing-memory-usage)
* [How Fil works](#how-fil-works)
    * [Fil and threading, with notes on NumPy and Zarr](#threading)
    * [What Fil tracks](#what-fil-tracks)

## Installation

Assuming you're on macOS or Linux, and are using Python 3.6 or later, you can use either Conda or pip (or any tool that is pip-compatible and can install `manylinux2010` wheels).

### Conda

To install on Conda:

```console
$ conda install -c conda-forge filprofiler
```

### Pip

To install the latest version of Fil you'll need Pip 19 or newer.
You can check like this:

```console
$ pip --version
pip 19.3.0
```

If you're using something older than v19, you can upgrade by doing:

```
$ pip install --upgrade pip
```

If _that_ doesn't work, try running that in a virtualenv.

Assuming you have a new enough version of pip:

```console
$ pip install filprofiler
```

## Using Fil

### <a name="peak-jupyter">Measuring peak (high-water mark) memory usage in Jupyter</a>

To measure memory usage of some code in Jupyter you need to do three things:

1. Use an alternative kernel, "Python 3 with Fil".
   You can choose this kernel when you create a new notebook, or you can switch an existing notebook in the Kernel menu; there should be a "Change Kernel" option in there in both Jupyter Notebook and JupyterLab.
2. Load the extension by doing `%load_ext filprofiler`.
3. Add the `%%filprofile` magic to the top of the cell with the code you wish to profile.


![Screenshot of JupyterLab](https://raw.githubusercontent.com/pythonspeed/filprofiler/master/images/jupyter.png)

### <a name="peak-python">Measuring peak (high-water mark) memory usage for Python scripts</a>

Instead of doing:

```console
$ python yourscript.py --input-file=yourfile
```

Just do:

```
$ fil-profile run yourscript.py --input-file=yourfile
```

And it will generate a report.

As of version 0.11, you can also run it like this:

```
$ python -m filprofiler run yourscript.py --input-file=yourfile
```

### <a name="oom">Debugging out-of-memory crashes</a>

First, run `free` to figure out how much memory is available—in this case about 6.3GB—and then set a corresponding limit on virtual memory with `ulimit`:

```console
$ free -h
       total   used   free  shared  buff/cache  available
Mem:   7.7Gi  1.1Gi  6.3Gi    50Mi       334Mi      6.3Gi
Swap:  3.9Gi  3.0Gi  871Mi
$ ulimit -Sv 6300000
```

Then, run your program under Fil, and it will generate a SVG at the point in time when memory runs out:

```console
$ fil-profile run oom.py 
...
=fil-profile= Wrote memory usage flamegraph to fil-result/2020-06-15T12:37:13.033/out-of-memory.svg
```

## <a name="reducing-memory-usage">Reducing memory usage in your code</a>

You've found where memory usage is coming from—now what?

If you're using data processing or scientific computing libraries, I have written a relevant [guide to reducing memory usage](https://pythonspeed.com/datascience/).

## How Fil works

Fil uses the `LD_PRELOAD`/`DYLD_INSERT_LIBRARIES` mechanism to preload a shared library at process startup.
This shared library captures all memory allocations and deallocations and keeps track of them.

At the same time, the Python tracing infrastructure (used e.g. by `cProfile` and `coverage.py`) to figure out which Python callstack/backtrace is responsible for each allocation.

### Fil and threading, with notes on NumPy and Zarr {#threading}

There are three cases:

* If you start a thread via Python, running Python code, that thread will get its own callstack for tracking who is responsible for a memory allocation.
* If you start a C thread, the calling Python code is considered responsible for any memory allocations in that thread.
* As a result, if you start a thread pool of threads that are not Python threads, the Python code that created those threads will be responsible for all allocations created during the thread pool's lifetime.
  This may lead to unexpected results.
  
In particular, both NumPy and Zarr create thread pools on startup that are used to run certain operations (BLAS linear algebra and compression, respectively).
Any allocations in the thread pool, while logically the responsibility of other code will be reported as due to importing those modules!

It's not clear to me they allocate a substantial amount of memory, so you might not have issues.
But, if you do have issues, one thing you can do is set these and some other libraries to not use a thread pool, by running the following code _before_ you import any Python or C libraries:

```python
# Disable multi-threaded backends in various scientific computing libraries
# (Zarr uses Blosc, NumPy uses BLAS, OpenMP is generically used):
from os import environ
environ["BLOSC_NTHREADS"] = "1"
environ["OMP_NUM_THREADS"] = "1"
environ["OPENBLAS_NUM_THREADS"] = "1"
environ["MKL_NUM_THREADS"] = "1"
environ["VECLIB_MAXIMUM_THREADS"] = "1"
environ["NUMEXPR_NUM_THREADS"] = "1"
```

The Python code calling into C code will then clearly be tracked as responsible for these allocations.

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
