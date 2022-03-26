# Limitations

## Limited reporting of tiny allocations

While every single allocation is tracked, for performance reasons only the largest allocations are reported, with a minimum of 99% of allocated memory reported.
The remaining <1% is highly unlikely to be relevant when trying to reduce usage; it's effectively noise.

## No support for subprocesses

This is planned, but not yet implemented.

## Missing memory allocation APIs

See the list in the page on [what Fil tracks](what-it-tracks.md).

## No support for third-party allocators

On Linux, Fil replaces the standard glibc allocator with [`jemalloc`](http://jemalloc.net/), though this is an implementation detail that may change in the future.

On all platforms, Fil will not work with custom allocators like `jemalloc` or `tcmalloc`.
