/* Help reproduce https://github.com/pythonspeed/filprofiler/issues/99 */

#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>

static void cleanup_handler(void* arg) {
  void* data = malloc(sizeof(int));
  printf("Allocated data! %ld\n", (long)data);
  free(data);
}

// A thread with thread-specific storage. When the thread exits, a cleanup
// function is called ono the thread-specific storage, and that cleanup function
// allocates some memory. Fil needs to handle allocations that happen during
// cleanup—they can't be tracked because the tracking code uses thread-local
// storage which doesn't exist anymore at this point!
static void* runs_in_thread(void *arg) {
  pthread_key_t thread_specific_storage;
  pthread_key_create(&thread_specific_storage, cleanup_handler);
  pthread_setspecific(thread_specific_storage, (void*)12);
  pthread_exit(NULL);
  return NULL;
}

void malloc_on_thread_exit() {
  pthread_t thread_id;
  void *result;
  pthread_create(&thread_id, NULL, runs_in_thread, NULL);
  pthread_join(thread_id, &result);
}
