// Python APIs that are not exposed in PyO3.

#include <pthread.h>
#include <Python.h>
#include <pystate.h>

pthread_t PyThreadState_GetPthreadId(PyThreadState* ts) {
  return ts->thread_id;
}
