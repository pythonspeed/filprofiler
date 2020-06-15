#include "Python.h"
#include "code.h"
#include "object.h"
#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif
#include "frameobject.h"
#include <dlfcn.h>
#include <malloc.h>
#include <stdint.h>
#include <stdio.h>
#include <sys/mman.h>

#define likely(x) __builtin_expect(!!(x), 1)
#define unlikely(x) __builtin_expect(!!(x), 0)

// Underlying APIs we're wrapping:
static void *(*underlying_real_malloc)(size_t length) = 0;
static void *(*underlying_real_calloc)(size_t nmemb, size_t length) = 0;
static void *(*underlying_real_realloc)(void *addr, size_t length) = 0;
static void (*underlying_real_free)(void *addr) = 0;

// Note whether we've been initialized yet or not:
static int initialized = 0;

// ID of Python code object extra data:
static Py_ssize_t extra_code_index = -1;

static _Thread_local int will_i_be_reentrant = 0;
// Current thread's Python state:
static _Thread_local PyFrameObject *current_frame = NULL;

// The file and function name responsible for an allocation.
struct FunctionLocation {
  const char *filename;
  Py_ssize_t filename_length;
  const char *function_name;
  Py_ssize_t function_name_length;
};

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
  underlying_real_realloc = dlsym(RTLD_NEXT, "realloc");
  if (!underlying_real_realloc) {
    fprintf(stderr, "Couldn't load realloc(): %s\n", dlerror());
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
                                    struct FunctionLocation *loc,
                                    uint16_t line_number);
extern void pymemprofile_finish_call();
extern void pymemprofile_new_line_number(uint16_t line_number);
extern void pymemprofile_reset();
extern void pymemprofile_dump_peak_to_flamegraph(const char *path);
extern void pymemprofile_add_allocation(size_t address, size_t length,
                                        uint16_t line_number);
extern void pymemprofile_free_allocation(size_t address);

void start_call(struct FunctionLocation *loc, uint16_t line_number) {
  if (!will_i_be_reentrant) {
    will_i_be_reentrant = 1;
    uint16_t parent_line_number = 0;
    if (current_frame != NULL && current_frame->f_back != NULL) {
      PyFrameObject *f = current_frame->f_back;
      parent_line_number = PyCode_Addr2Line(f->f_code, f->f_lasti);
    }
    pymemprofile_start_call(parent_line_number, loc, line_number);
    will_i_be_reentrant = 0;
  }
}

void finish_call() {
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

__attribute__((visibility("default"))) void fil_reset(const char* default_path) {
  if (!will_i_be_reentrant) {
    will_i_be_reentrant = 1;
    pymemprofile_reset(default_path);
    will_i_be_reentrant = 0;
  }
}

__attribute__((visibility("default"))) void
fil_dump_peak_to_flamegraph(const char *path) {
  // This maybe called after we're done, when will_i_be_reentrant is permanently
  // set to 1, or might be called mid-way through code run. Either way we want
  // to prevent reentrant malloc() calls, but we want to run regardless.
  int current_reentrant_status = will_i_be_reentrant;
  will_i_be_reentrant = 1;
  pymemprofile_dump_peak_to_flamegraph(path);
  will_i_be_reentrant = current_reentrant_status;
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

__attribute__((visibility("default"))) void *realloc(void *addr, size_t size) {
  if (unlikely(!initialized)) {
    fprintf(stderr, "BUG: We don't handle realloc() during initialization.\n");
    abort();
  }
  void *result = underlying_real_realloc(addr, size);
  if (!will_i_be_reentrant && initialized) {
    will_i_be_reentrant = 1;
    // Sometimes you'll get same address, so if we did remove first and then
    // added, it would remove the entry erroneously.
    pymemprofile_free_allocation((size_t)addr);
    add_allocation((size_t)result, size);
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

// Call after Python gets going.
__attribute__((visibility("default"))) void fil_initialize_from_python() {
  extra_code_index = _PyEval_RequestCodeExtraIndex(NULL);
}

__attribute__((visibility("hidden"))) int
fil_tracer(PyObject *obj, PyFrameObject *frame, int what, PyObject *arg) {
  switch (what) {
  case PyTrace_CALL:
    // Store the current frame, so malloc() can look up line number:
    current_frame = frame;

    /*
      We want an efficient identifier for filename+fuction name. So we:

      1. Incref the two string objects so they never get GC'ed.
      2. Store references to the corresponding UTF8 strings on the code object
         as extra info.

      The pointer address of the resulting struct can be used as an
      identifier.
    */
    struct FunctionLocation *loc = NULL;
    assert(extra_code_index != -1);
    _PyCode_GetExtra((PyObject *)frame->f_code, extra_code_index,
                     (void **)&loc);
    if (loc == NULL) {
      // Ensure the two string never get garbage collected;
      Py_INCREF(frame->f_code->co_filename);
      Py_INCREF(frame->f_code->co_name);
      loc = underlying_real_malloc(sizeof(struct FunctionLocation));
      loc->filename = PyUnicode_AsUTF8AndSize(frame->f_code->co_filename,
                                              &loc->filename_length);
      loc->function_name = PyUnicode_AsUTF8AndSize(frame->f_code->co_name,
                                                   &loc->function_name_length);
      _PyCode_SetExtra((PyObject *)frame->f_code, extra_code_index,
                       (void *)loc);
    }
    start_call(loc, frame->f_lineno);
    break;
  case PyTrace_RETURN:
    finish_call();
    // We're done with this frame, so set the parent frame:
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
