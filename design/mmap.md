# Notes on `mmap()`

## `malloc()` can `mmap()`

Behind the scenes, `malloc()` might call `mmap()` when asked to allocate large chunks of memory.
Because we have reentrancy control, that `mmap()` will _not_ be captured by the allocation tracking code, which is what we want.
And the tracking code will think of it as a normal `malloc()` (as it should, use of `mmap()` is implementation detail).
