#include <stdint.h>
#include <stdlib.h>

extern "C" {
void *cppnew() { return new uint32_t[10485760]; }
}
