# Fil4prod vs open source Fil

Fil is an open source memory profiler for Python; Fil4prod is a proprietary memory profiler for Python.
The difference is in their focus and capabilities.

## Fil: offline memory profiling

Fil's goal is to help data scientists, scientists, data engineers, and programmers to identify memory usage problems in their code.
In order to do so, it tracks _every single memory allocation_ it can, large and small, and tries to make the profiling results as accurate as possible.

While this means Fil works well catching even small memory problems or leaks, it has downsides too:

* Tracking all memory allocations has a performance cost.
* In order to ensure good reporting Fil makes some [changes to how certain libraries run](../fil/threadpool-disabled.md).
* Fil's mechanism for catching out-of-memory problems is a useful heuristic, but as a heuristic it could adversely impact production jobs.

Overall, Fil aims to give the best possible information, at the cost of performance and behavior differences from uninstrumented code.

## Fil4prod: memory profiling in production

Limiting memory profiling to developer machines has its limitations:

* Some problems only occur in production.
* If a process takes 64GB of RAM and 12 hours to run, reproducing the problem locally can be difficult and slow.

Ideally, _every single production job_ would have memory profiling enabled by default, just in case.
If memory usage is too high, you won't have to go back and rerun it, you'd have a profiling report already prepared.

In order to achieve this, Fil4prod emphasizes speed over accuracy.
In particular, Fil4prod uses sampling, only tracking a subset of memory allocations.
While this means much lower performance overhead, it has some caveats:

* Results will only be useful for processes that allocate large amounts of memory; 500MB of RAM or more, say.
* Reported callstacks for the smallest allocations may be wrong.

In practice, for data-intensive batch jobs with high memory usage, both these caveats are irrelevant.
If you're trying to figure out why you're using 16GB of RAM, you'll care about the multi-gigabyte or 100s-of-megabyte sources of allocation, and the fact that a 1MB allocation is reporting the wrong callstack doesn't really matter.

**If you'd like to get memory profiling automatically for all your production batch jobs, [send me an email](mailto:itamar@pythonspeed.com) to participate in the alpha program.**
