# Writing software that's reliable enough for production

How do you write software that's reliable enough to run in production?
Fil4prod is a Python memory profiler intended for always-on profiling of production data processing jobs, and so I had to address this problem to my satisfaction.

Here is what I did.

<!-- toc -->

## Programming language: Rust

Writing a memory profiler has certain constraints:

1. It needs to be fast, since it will be running in a critical code path.
2. The language probably shouldn't use garbage collection, both for performance reasons and since memory reentrancy issues are one of the more annoying causes of problems in memory profilers.

To expand on reentrancy: if you're capturing `malloc()` calls, having the profiler then call `malloc()` itself is both a performance problem and a potential for recursively blowing up the stack.
So it's best to know exactly when allocation and memory freeing happens so that it can be done safely.

Rust fulfills both these criteria, but comes with many other benefits compared to C or C+:

* Memory safety and thread safety.
* Required branch coverage.
  If for example, you're handling an enum, Rust's compiler will ensure you handle all cases.
* Similarly, `Result` objects (the main way to get errors) must be handled, you can't just drop them on the floor.
* No `NULL` or `nil`.
* Panics in threads don't take down the whole process.

### Caveat: `unsafe` in libraries

Of course, Rust has an escape hatch from its safety model: `unsafe`.
Third party libraries that Fil4prod depends on can use this to cause undefined behavior bugs much like C/C++ code.

I am trying to mitigate this by choosing popular libraries that have had some real-world testing, but longer term might also change choice of libraries.

### Caveat: `unsafe` in Fil4prod

The other problem is that Fil4prod spends much of its time talking to C APIs: for memory allocation, and to deal with CPython interpreter internals.
Doing so inherently requires opting out of Rust's safety.
This is mitigated by providing safe wrappers around `unsafe` APIs.

### Caveat: Rust limitations

* Rust's thread locals aren't quite sufficient ([they're slow](https://matklad.github.io/2020/10/03/fast-thread-locals-in-rust.html) and allow using them can allocate memory, which means reentrancy).
* Implementing C-style variable argument is not yet supported (this is necessary for capturing `mremap()`).

Hopefully both issues will be fixed; for now there's a tiny bit of C code.

## Prototyping

The [open source Fil profiler](https://pythonspeed.com/fil/) acted as a prototype for Fil4prod.
By writing Fil first:

* I was able to spot potential issues in advance (sometimes by encountering them in the wild).
* Some of the code is shared between, and as a result has had real-world testing before Fil4prod was even released.
* Fil4prod is in some ways a redesign, based on lessons learned from Fil.

## Automated testing

Fil4prod has plenty of automated tests, both API-level unit tests and end-to-end tests.
Some points worth covering:

### Coverage marks

One useful technique when testing is coverage markers: the ability to mark a certain branch in the code, and then have a test assert "_that_ branch was called in this test."
Much of what Fil4prod does is pretending to be exactly the same as normal `malloc()` while doing something slightly different internally for tracking purposes—coverage markers make sure black box test are hitting the right code path.

For more details [see here](https://ferrous-systems.com/blog/coverage-marks/).

### Property-based testing

When possible, property-based testing is used to generate a wide variety of test cases automatically.

### End-to-end tests

Fil4prod is designed to run inside a Python process, so for reliable testing it is critical to have tests that run a full Python program with Fil4prod injected.
The flawed "test pyramid" notion of lots of unit tests and only a tiny number of end-to-end tests doesn't apply in this particular situation: it's necessary to have plenty of both.

## Environmental assertions on startup

Fil4prod has certain environmental invariants: for example, matching the appropriate version of Python.
Serving in its admirable role as a prototype, Fil had a build system bug where code compiled against Python version 3.6 was packaged for Python version 3.9, leading to segfaults.

In addition, for performance reasons Fil4prod sometimes requires [transgressive programming](https://pythonspeed.com/articles/transgressive-programming/), violating abstraction boundaries and relying on internal details of glibc and CPython.
These details are only likely to change every few years, with a major release, so it's highly unlikely users will encounter them, but this is still a risk factor.

To prevent mysterious crashes, all of these invariants are tested on startup.
If the checks fail, Fil4prod will cause the program to exit early, and critically with a useful error message.
This is much better than segfaulting later in some arbitrary part of user code, which is both hard to debug and could in theory lead to corrupted user data.

## Dependency due diligence

In selecting libraries to depend on, I follow the usual checklist of good maintenance, large user base, and so on.
But there are also some automated tests, in particular using [Rust's advisory database](https://rustsec.org/) to ensure no dependencies have security advisories, soundness issues, or are unmaintained.

## Next steps

### User testing

There's only so far internal processes can get you: testing software in production is in the end the only way to find certain problems.
If you'd like to get memory profiles for all your batch jobs, [send me an email](mailto:itamar@pythonspeed.com) to participate in alpha program.

### Rudra

[Rudra](https://github.com/sslab-gatech/Rudra) is a static analyzer for Rust that can catch certain unsoundness issues in `unsafe` code.

### Better handling of errors in shutdown report generation

Right now if Fil4prod fails while writing out the final report, I suspect it will either freeze or crash.
What should actually happen is that the process exits as normal, profiling should not interfere with shutdown.

### Panic injection testing

Fil4prod delegates some of its processing to a thread.
If that thread panics the Python program should continue running, unaffected, although there won't be a memory profiling report dumped at the end.
This is not tested at the moment.

### Lints to catch APIs that shouldn't be used

Certain APIs in Rust will panic if the data is in an unexpected state, e.g. `Option<T>::unwrap`.
While I am manually avoiding them, it might be useful to have lints to catch usage of these APIs.

### Try to reduce `unsafe` in third-party libraries

All things being equal, a library using `unsafe` is more likely to have unsoundness bugs than a library that doesn't use `safe`.
It may therefore be possible to switch some of Fil4prod's dependencies to safer alternatives.
