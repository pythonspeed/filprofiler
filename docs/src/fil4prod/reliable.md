# Writing software that's reliable enough for production

How do you write software that's reliable enough to run in production?

Fil4prod is a Python memory profiler intended for always-on profiling of production data processing jobs.
Critically, Fil4prod runs _inside_ the process that is being profiled.
As a result, failure in Fil4prod can in theory crash the user's program, or even corrupt data.
This clearly, is unacceptable.

As I implemented Fil4prod I had to address this problem to my satisfaction.
Here is what I have done, and what I plan to do next.

<!-- toc -->

## Guiding principles

1. **Do no harm.** Failures in Fil4prod should not affect the running program.
2. **Fail fast.** If breaking the running program is unavoidable, fail as early as possible, and with a meaningful error.

## Choice of programming language: Rust

Writing a memory profiler has certain constraints:

1. It needs to be fast, since it will be running in a critical code path.
2. The language probably shouldn't use garbage collection, both for performance reasons and since memory reentrancy issues are one of the more annoying causes of problems in memory profilers.

To expand on reentrancy: if you're capturing `malloc()` calls, having the profiler then call `malloc()` itself is both a performance problem and a potential for recursively blowing up the stack.
So it's best to know exactly when allocation and memory freeing happens so that it can be done safely.

Rust fulfills both these criteria, but comes with many other benefits compared to C or C++:

* Memory safety and thread safety.
* Enforces handling all possible values of enums; Rust's compiler will complain if you don't handle all cases.
* Similarly, `Result` objects (the main way to get errors) must be handled, you can't just drop them on the floor.
* No `NULL` or `nil`; there's `Option<T>`, but the branch coverage requirement means both cases will be handled, and it's explicitly nullable, vs. e.g. C or C++ where any pointer can be `NULL`.

### Caveat: `unsafe` in libraries

Rust has an escape hatch from its safety model: `unsafe`.
Third-party libraries that Fil4prod depends on can use this to cause undefined behavior bugs, much like C/C++ code.

I am trying to mitigate this by choosing popular libraries that have had some real-world testing, but longer term might also change my choice of libraries.

### Caveat: `unsafe` in Fil4prod

Fil4prod spends much of its time talking to C APIs: for memory allocation, and to deal with CPython interpreter internals.
Doing so inherently requires opting out of Rust's safety.

This is mitigated by providing safe wrappers around `unsafe` APIs.
For example, to ensure I'm not passing around a pointer that might be `NULL`, I could do:

```rust
/// Wrapper around void* that maps to an allocation from libc.
pub struct Allocation {
    // If pointer is NULL this will be `None`, otherwise `Some(pointer)`.
    pointer: Option<*mut c_void>,
}

impl Allocation {
    // Wrap a new pointer.
    pub fn wrap(pointer: *mut c_void) -> Self {
        let pointer = if pointer.is_null() {
            None
        } else {
            Some(pointer)
        };
        Self { pointer }
    }
    
    pub fn malloc(size: usize) -> Self {
        Self::wrap(unsafe { libc::malloc(size) })
    }

    // ... other APIs
}
```

The use of `Option` means any time I try to get at the underlying pointer, Rust's compiler will complain if the `None` case isn't handled, so long as the `unwrap()` and `expect()` APIs aren't used.
(An even more succinct implementation would use [`std::ptr::NonNull::new()`](https://doc.rust-lang.org/std/ptr/struct.NonNull.html#method.new).)

One approach I haven't taken is trying [Miri](https://github.com/rust-lang/miri), a tool that will catch some bugs in `unsafe` code.
From the documentation, it seems like it won't work with FFI.
Since FFI is the only reason Fil4prod uses `unsafe`, it seems like Miri would be both be difficult or impossible to use, and not particularly helpful.

### Caveat: Rust limitations

* Rust's thread locals aren't quite sufficient ([they're slow](https://matklad.github.io/2020/10/03/fast-thread-locals-in-rust.html) and using them can allocate memory, which means reentrancy).
* Implementing C-style variable arguments to function is not yet supported; this is necessary for capturing `mremap()`.

Hopefully both issues will be fixed in stable Rust; for now there's a tiny bit of C code required.

## Preventing panics in Rust

Certain APIs in Rust will panic if the data is in an unexpected state, e.g. `Option<T>::unwrap()` will panic if the value is `None`.
Unlike a segfault, panics are thread-specific and can be recovered from.
But while panics in Fil's internal thread can be handled gracefully, panics in the application threads could take down the whole program if they hit FFI boundaries.
The goal then is to avoid panics as much as possible.

Sometimes this is done by using non-panicking APIs.
In the case of `Option<T>`, there are other APIs to extract `T` that will not panic.

In other cases, panics can be avoided by appropriate error handling.
In a normal program, shutting down might be fine if log initialization fails, but Fil4prod should just keep running and live without logs.

In order to enforce a lack of panics, the [Clippy linter](https://github.com/rust-lang/rust-clippy/) is used to catch Rust APIs that can cause panics.
Normal integer arithmetic is also avoided to avoid bugs caused by overflows; saturating APIs are used instead.

```rust
#![deny(
    clippy::expect_used,
    clippy::unwrap_used,
    clippy::ok_expect,
    clippy::integer_division,
    clippy::indexing_slicing,
    clippy::integer_arithmetic,
    clippy::panic,
    clippy::match_on_vec_items,
    clippy::manual_strip,
    clippy::await_holding_refcell_ref
)]
```

Clippy is run as part of CI.

Additionally [`assert_panic_free`](https://docs.rs/assert_panic_free) is used to assert that there are no panics in critical code paths (see also: [`no_panic`](https://docs.rs/no-panic/), [`dont_panic`](https://github.com/Kixunil/dont_panic), [`panic-never`](https://github.com/japaric/panic-never)).
Unfortunately the way it works is not ideal, insofar as it doesn't identify which particular code had a panic, and since it can't always deduce correctly whether code is panic free.

### When panics might happen anyway

Given the use of third-party libraries, there are parts of the code where it's much harder to prove that panics are impossible.
In these situations, I use [`std::panic::catch_unwind`](https://doc.rust-lang.org/std/panic/fn.catch_unwind.html) to catch any panics that might occur.

In addition, a [panic hook](https://doc.rust-lang.org/std/panic/fn.set_hook.html) is used to disable profiling on panics; when this happens, the user's Python program should hopefully continue as normal.

## Prototyping

The [open source Fil profiler](https://pythonspeed.com/fil/) acted as a prototype for Fil4prod.
By writing Fil first:

* I was able to spot potential issues in advance (sometimes by encountering them in the wild).
* Some of the code is shared between the two, and as a result has had real-world testing before Fil4prod was even released.
* Fil4prod is in some ways a redesign, based on lessons learned from Fil.

## Automated testing

Fil4prod has plenty of automated tests, both low-level unit tests and end-to-end tests.
Some points worth covering:

### Coverage marks

One useful technique when testing is coverage mark: the ability to mark a certain branch in the code, and then have a test assert "_that_ branch was called in this test."
Much of what Fil4prod does is pretending to be exactly the same as normal `malloc()` while doing something slightly different internally for tracking purposes.
Coverage marks allow me to ensure black-box tests are hitting the right code path.

For more details [see here](https://ferrous-systems.com/blog/coverage-marks/).

### Property-based testing

When possible, property-based testing is used to generate a wide variety of test cases automatically.
I'm using the [`proptest`](https://docs.rs/proptest/) library for Rust.

### End-to-end tests

Fil4prod is designed to run inside a Python process, so for reliable testing it is critical to have tests that run a full Python program with Fil4prod injected.
The flawed "test pyramid" notion of lots of unit tests and only a tiny number of end-to-end tests doesn't apply in this particular situation: it's necessary to have plenty of both.

### Contracts and debug assertions

Fil4prod uses [pre- and post-contracts](https://docs.rs/contracts/), plus debug assertions, to ensure invariants are being followed.
Of course, these are disabled in the release build for performance reasons.
So to ensure correctness, the end-to-end tests are actually run twice: once with the release build, and once with debug assertions enabled.

### Panic injection testing

Some of Fil4prod's test make certain "failpoints" panic, using a technique similar to [`fail`](https://docs.rs/fail/).
This allows testing that unexpected failures in Fil4prod won't impact the running program.

## Environmental assertions on startup

Fil4prod has certain environmental invariants: for example, matching the appropriate version of Python.
This can happen: the open source version of Fil had a build system bug where code compiled against Python 3.6 was packaged for Python 3.9, leading to segfaults.

In addition, for performance reasons Fil4prod sometimes requires [transgressive programming](https://pythonspeed.com/articles/transgressive-programming/), violating abstraction boundaries and relying on internal details of glibc and CPython.
These details are only likely to change every few years, with a major release, so it's highly unlikely users will encounter them, but this is still a risk factor.

To prevent mysterious crashes, all of these invariants are tested on startup.
If the checks fail, Fil4prod will cause the program to exit early with a useful error message.
This is much better than segfaulting later in some arbitrary part of user code, which is both hard to debug and could in theory lead to corrupted user data.

## Dependency due diligence

In selecting libraries to depend on, I try to pick reasonable dependencies; for example, all other things being equal, a library with a large user base is likely better than a library almost no one uses.
But there are also some automated tests, in particular using [Rust's advisory database](https://rustsec.org/) to ensure no dependencies have known security advisories, soundness issues, or are unmaintained.

## Next steps, a partial list

### User testing

There's only so far internal processes can get you: testing software in production is in the end the only way to find certain problems.
**If you'd like to get memory profiling automatically for all your production batch jobs, [send me an email](mailto:itamar@pythonspeed.com) to participate in the alpha program.**

### Rudra

[Rudra](https://github.com/sslab-gatech/Rudra) is a static analyzer for Rust that can catch certain unsoundness issues in `unsafe` code.
I should run it on Fil4prod.

### Other potential approaches to panic reduction

[`findpanics`](https://github.com/philipc/findpanics) is a tool that finds panics using binary analysis of compiled code.
[`rustig`](https://github.com/Technolution/rustig) is similar, but seems even less maintained.

### Try to reduce `unsafe` in third-party libraries

All other things being equal, a library using `unsafe` is more likely to have unsoundness bugs than a library that doesn't use `safe`.
It may be possible to switch some of Fil4prod's dependencies to safer alternatives.
