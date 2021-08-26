# Debugging out-of-memory crashes using Fil

Typically when your program runs out of memory, it will crash, or get killed mysteriously by the operating system, or [other unfortunate side-effects](https://pythonspeed.com/articles/python-out-of-memory/).

To help you debug these problems, Fil will heuristically try to catch out-of-memory conditions, and dump a report if thinks your program is out of memory.
It will then exit with exit code 53.

```console
$ fil-profile run oom.py 
...
=fil-profile= Wrote memory usage flamegraph to fil-result/2020-06-15T12:37:13.033/out-of-memory.svg
```

Fil uses three heuristics to determine if the process is close to running out of memory:

* A failed allocation, indicating insufficient memory is available.
* The operating system or memory-limited cgroup (e.g. a Docker container) only has 100MB of RAM available.
* The process swap is larger than available memory, indicating heavy swapping by the process.
  In general you want to avoid swapping, and e.g. [explicitly use `mmap()`](https://pythonspeed.com/articles/mmap-vs-zarr-hdf5/) if you expect to be using disk as a backfill for memory.

For a more detailed example of out-of-memory detection with Fil, see this article on [debugging out-of-memory crashes](https://pythonspeed.com/articles/crash-out-of-memory/).

#### Disabling the out-of-memory detection

Sometimes the out-of-memory detection heuristic will kick in too soon, shutting down the program even though in practice it could finish running.
You can disable the heuristic by doing:

```console
fil-profile --disable-oom-detection run yourprogram.py
```
