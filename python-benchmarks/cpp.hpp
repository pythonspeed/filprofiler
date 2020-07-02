#include <stdint.h>

void* cppnew() {
  return new uint32_t[10485760];
}
