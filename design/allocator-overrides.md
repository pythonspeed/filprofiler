# Allocator overrides

Fil works by overriding `malloc`, `calloc`, `realloc` and `free`.
Allocations are then handed to tracking code (written in Rust) that keeps track of what is allocated.

## Injecting new `malloc`

### Linux

1. The shared library has its own `malloc`, `free`, etc..
2. It is preloaded using `LD_PRELOAD`.
3. The underlying `malloc` are loaded as function pointers using `dlsym(RTLD_NEXT)`.

### macOS

1. The shared library has `reimplemented_malloc`, `reimplemented_free`, etc..
2. The symbols are set to override `malloc` etc. by using `DYLD_INTERPOSE(reimplemented_malloc, malloc)`.
3. The shared library is reploaded using `DYLD_INSERT_LIBRARIES`.

The alternative would be #3 plus setting a flat namespace, but having the ability to easily refer to underlying `malloc` is nice.

The code code either call `malloc()` as normal, or use the same `dlsym()` function pointers as Linux does.
At the moment it does the latter, but that could be changed.

## Preventing reentrancy

In order to prevent allocations from the Rust code being recursively tracked, a thread-local is used to prevent reentrant calls to `malloc` and friends.

* Linux: a `static _Thread_local` is used.
* macOS: pthreads keys are used, since `_Thread_local` results in use of `malloc()` and therefore infinite recursion.

