# Fil vs other Python memory tools

There are two distinct patterns of Python usage, each with its own source of memory problems.

In a long-running server, memory usage can grow indefinitely due to memory leaks.
That is, some memory is not being freed.

* If the issue is in Python code, tools like [`tracemalloc`](https://docs.python.org/3/library/tracemalloc.html) and [Pympler](https://pypi.org/project/Pympler/) can tell you which objects are leaking and what is preventing them from being leaked.
* If you're leaking memory in C code, you can use tools like [Valgrind](https://valgrind.org).

Fil, however, is not specifically aimed at memory leaks, but at the other use case: data processing applications.
These applications load in data, process it somehow, and then finish running.

The problem with these applications is that they can, on purpose or by mistake, allocate huge amounts of memory.
It might get freed soon after, but if you allocate 16GB RAM and only have 8GB in your computer, the lack of leaks doesn't help you.

Fil will therefore tell you, in an easy to understand way:

1. Where peak memory usage is, also known as the high-water mark.
2. What code was responsible for allocating the memory that was present at that peak moment.
3. This includes C/Fortran/C++/whatever extensions that don't use Python's memory allocation API (`tracemalloc` only does Python memory APIs).

This allows you to [optimize that code in a variety of ways](https://pythonspeed.com/memory/).
