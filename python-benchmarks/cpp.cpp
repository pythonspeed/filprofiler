#include <stdint.h>
#include <stdlib.h>

extern "C" {
void *cppnew() { return new uint32_t[10485760]; }

// For some reason Cython-generated code fails to find aligned_alloc() when
// compiling on macOS, so we use C++ directly.
void aligned_alloc_wrapper() { aligned_alloc(64, 1024 * 1024 * 90); }
}
