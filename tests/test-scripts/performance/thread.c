#include <pthread.h>
#include <stdint.h>
#include <stdlib.h>
#include <unistd.h>

void *in_thread(void *ignore) { sleep(1); }

void sleep_in_thread() {
  pthread_t thread_id;
  void *result;
  pthread_create(&thread_id, NULL, in_thread, NULL);
}
