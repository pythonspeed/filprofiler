#include <pthread.h>
#include <stdint.h>
#include <stdlib.h>

extern "C" {
void *cppnew() { return new uint32_t[10485760]; }

void *in_thread(void *ignore) { return malloc(1024 * 1024 * 17); }

void allocate_in_thread() {
  pthread_t thread_id;
  void *result;
  pthread_create(&thread_id, NULL, in_thread, NULL);
  pthread_join(thread_id, &result);
}
}
