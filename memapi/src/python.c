// Python APIs that are not exposed in PyO3.

#include <Python.h>
#include <pystate.h>
#include <stdint.h>

uint64_t PyThreadState_GetNativeThreadId(PyThreadState* ts) {
  return ts->native_thread_id;
}
