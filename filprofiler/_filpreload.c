#include "Python.h"
#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif
#include "frameobject.h"
#include <dlfcn.h>
#include <malloc.h>
#include <stdint.h>
#include <stdio.h>
#include <sys/mman.h>

#if PY_VERSION_HEX < 0x03080000
extern PyAPI_FUNC(int) _Py_UnixMain(int argc, char **argv);
#define Py_BytesMain _Py_UnixMain
#endif

#define likely(x) __builtin_expect(!!(x), 1)
#define unlikely(x) __builtin_expect(!!(x), 0)

// Underlying APIs we're wrapping:
static void *(*underlying_real_malloc)(size_t length) = 0;
static void *(*underlying_real_calloc)(size_t nmemb, size_t length) = 0;
static void (*underlying_real_free)(void *addr) = 0;

// Note whether we've been initialized yet or not:
static int initialized = 0;

static _Thread_local int will_i_be_reentrant = 0;
// Current thread's Python state:
static _Thread_local PyFrameObject *current_frame = NULL;

static void __attribute__((constructor)) constructor() {
  if (initialized) {
    return;
  }

  if (sizeof((void *)0) != sizeof((size_t)0)) {
    fprintf(stderr, "BUG: expected size of size_t and void* to be the same.\n");
    exit(1);
  }
  underlying_real_malloc = dlsym(RTLD_NEXT, "malloc");
  if (!underlying_real_malloc) {
    fprintf(stderr, "Couldn't load malloc(): %s\n", dlerror());
    exit(1);
  }
  underlying_real_calloc = dlsym(RTLD_NEXT, "calloc");
  if (!underlying_real_calloc) {
    fprintf(stderr, "Couldn't load calloc(): %s\n", dlerror());
    exit(1);
  }
  underlying_real_free = dlsym(RTLD_NEXT, "free");
  if (!underlying_real_free) {
    fprintf(stderr, "Couldn't load free(): %s\n", dlerror());
    exit(1);
  }

  initialized = 1;
  unsetenv("LD_PRELOAD");
}

extern void *__libc_malloc(size_t size);
extern void *__libc_calloc(size_t nmemb, size_t size);
extern void pymemprofile_start_call(uint16_t parent_line_number,
                                    const char *filename, const char *funcname,
                                    uint16_t line_number);
extern void pymemprofile_finish_call();
extern void pymemprofile_new_line_number(uint16_t line_number);
extern void pymemprofile_reset();
extern void pymemprofile_dump_peak_to_flamegraph(const char *path);
extern void pymemprofile_add_allocation(size_t address, size_t length,
                                        uint16_t line_number);
extern void pymemprofile_free_allocation(size_t address);

__attribute__((visibility("default"))) void
fil_start_call(const char *filename, const char *funcname,
               uint16_t line_number) {
  if (!will_i_be_reentrant) {
    will_i_be_reentrant = 1;
    uint16_t parent_line_number = 0;
    if (current_frame != NULL && current_frame->f_back != NULL) {
      PyFrameObject *f = current_frame->f_back;
      parent_line_number = PyCode_Addr2Line(f->f_code, f->f_lasti);
    }

    pymemprofile_start_call(parent_line_number, filename, funcname,
                            line_number);
    will_i_be_reentrant = 0;
  }
}

__attribute__((visibility("default"))) void fil_finish_call() {
  if (!will_i_be_reentrant) {
    will_i_be_reentrant = 1;
    pymemprofile_finish_call();
    will_i_be_reentrant = 0;
  }
}

__attribute__((visibility("default"))) void
fil_new_line_number(uint16_t line_number) {
  if (!will_i_be_reentrant) {
    will_i_be_reentrant = 1;
    pymemprofile_new_line_number(line_number);
    will_i_be_reentrant = 0;
  }
}

__attribute__((visibility("default"))) void fil_reset() {
  if (!will_i_be_reentrant) {
    will_i_be_reentrant = 1;
    pymemprofile_reset();
    will_i_be_reentrant = 0;
  }
}

__attribute__((visibility("default"))) void
fil_dump_peak_to_flamegraph(const char *path) {
  if (!will_i_be_reentrant) {
    will_i_be_reentrant = 1;
    pymemprofile_dump_peak_to_flamegraph(path);
    will_i_be_reentrant = 0;
  }
}

__attribute__((visibility("hidden"))) void add_allocation(size_t address,
                                                          size_t size) {
  uint16_t line_number = 0;
  PyFrameObject *f = current_frame;
  if (f != NULL) {
    line_number = PyCode_Addr2Line(f->f_code, f->f_lasti);
  }
  pymemprofile_add_allocation(address, size, line_number);
}

// Override memory-allocation functions:
__attribute__((visibility("default"))) void *malloc(size_t size) {
  if (unlikely(!initialized)) {
    return mmap(NULL, size, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS,
                -1, 0);
  }
  void *result = underlying_real_malloc(size);
  if (!will_i_be_reentrant && initialized) {
    will_i_be_reentrant = 1;
    add_allocation((size_t)result, size);
    will_i_be_reentrant = 0;
  }
  return result;
}

__attribute__((visibility("default"))) void *calloc(size_t nmemb, size_t size) {
  if (unlikely(!initialized)) {
    return mmap(NULL, nmemb * size, PROT_READ | PROT_WRITE,
                MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
  }
  void *result = underlying_real_calloc(nmemb, size);
  size_t allocated = nmemb * size;
  if (!will_i_be_reentrant && initialized) {
    will_i_be_reentrant = 1;
    add_allocation((size_t)result, allocated);
    will_i_be_reentrant = 0;
  }
  return result;
}

__attribute__((visibility("default"))) void free(void *addr) {
  if (unlikely(!initialized)) {
    // Well, we're going to leak a little memory, but, such is life...
    return;
  }
  underlying_real_free(addr);
  if (!will_i_be_reentrant) {
    will_i_be_reentrant = 1;
    pymemprofile_free_allocation((size_t)addr);
    will_i_be_reentrant = 0;
  }
}

__attribute__((visibility("hidden"))) int
fil_tracer(PyObject *obj, PyFrameObject *frame, int what, PyObject *arg) {
  switch (what) {
  case PyTrace_CALL:
    current_frame = frame;
    fil_start_call(PyUnicode_AsUTF8(frame->f_code->co_filename),
                   PyUnicode_AsUTF8(frame->f_code->co_name), frame->f_lineno);
    break;
  case PyTrace_RETURN:
    fil_finish_call();
    current_frame = frame->f_back;
    break;
  default:
    break;
  }
  return 0;
}

__attribute__((visibility("default"))) void register_fil_tracer() {
  PyEval_SetProfile(fil_tracer, Py_None);
}

__attribute__((visibility("default"))) void fil_shutting_down() {
  // We're shutting down, so things like PyCode_Addr2Line won't work:
  will_i_be_reentrant = 1;
}
