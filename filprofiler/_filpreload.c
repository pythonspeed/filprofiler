#include "Python.h"
#include "code.h"
#include "object.h"
#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif
#include "frameobject.h"
#include <dlfcn.h>
#include <stdint.h>
#include <stdio.h>
#include <sys/mman.h>

#ifdef __APPLE__
#define SYMBOL_PREFIX(func) reimplemented_##func
#else
#define SYMBOL_PREFIX(func) func
#endif

#define likely(x) __builtin_expect(!!(x), 1)
#define unlikely(x) __builtin_expect(!!(x), 0)

// Underlying APIs we're wrapping:
static void *(*underlying_real_malloc)(size_t length) = 0;
static void *(*underlying_real_calloc)(size_t nmemb, size_t length) = 0;
static void *(*underlying_real_realloc)(void *addr, size_t length) = 0;
static void (*underlying_real_free)(void *addr) = 0;
static void *(*underlying_real_mmap)(void *addr, size_t length, int prot,
                                     int flags, int fd, off_t offset) = 0;
static int (*underlying_real_munmap)(void *addr, size_t length) = 0;

// Note whether we've been initialized yet or not:
static int initialized = 0;

// ID of Python code object extra data:
static Py_ssize_t extra_code_index = -1;

#ifdef __APPLE__
#include "interpose.h"
#include <pthread.h>
static pthread_key_t will_i_be_reentrant;
static pthread_once_t will_i_be_reentrant_once = PTHREAD_ONCE_INIT;

static void make_pthread_key() {
  pthread_key_create(&will_i_be_reentrant, (void *)0);
}

static inline uint64_t am_i_reentrant() {
  (void)pthread_once(&will_i_be_reentrant_once, make_pthread_key);
  return (int)pthread_getspecific(will_i_be_reentrant);
}

static inline void set_will_i_be_reentrant(uint64_t i) {
  pthread_setspecific(will_i_be_reentrant, (void *)i);
}
#else
#include <sys/syscall.h>
static _Thread_local int will_i_be_reentrant = 0;

static inline int am_i_reentrant() { return will_i_be_reentrant; }

static inline void set_will_i_be_reentrant(int i) { will_i_be_reentrant = i; }
#endif

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
  underlying_real_mmap = dlsym(RTLD_NEXT, "mmap");
  if (!underlying_real_mmap) {
    fprintf(stderr, "Couldn't load mmap(): %s\n", dlerror());
    exit(1);
  }
  underlying_real_munmap = dlsym(RTLD_NEXT, "munmap");
  if (!underlying_real_munmap) {
    fprintf(stderr, "Couldn't load munmap(): %s\n", dlerror());
    exit(1);
  }

  initialized = 1;
  unsetenv("LD_PRELOAD");
  // This seems to break things... revisit at some point.
  // unsetenv("DYLD_INSERT_LIBRARIES");
}

// Implemented in the Rust library:
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
extern void pymemprofile_add_anon_mmap(size_t address, size_t length,
                                       uint16_t line_number);
extern void pymemprofile_free_anon_mmap(size_t address, size_t length);

void start_call(struct FunctionLocation *loc, uint16_t line_number) {
  if (!am_i_reentrant()) {
    set_will_i_be_reentrant(1);
    uint16_t parent_line_number = 0;
    if (current_frame != NULL && current_frame->f_back != NULL) {
      PyFrameObject *f = current_frame->f_back;
      parent_line_number = PyCode_Addr2Line(f->f_code, f->f_lasti);
    }
    pymemprofile_start_call(parent_line_number, loc, line_number);
    set_will_i_be_reentrant(0);
  }
}

void finish_call() {
  if (!am_i_reentrant()) {
    set_will_i_be_reentrant(1);
    pymemprofile_finish_call();
    set_will_i_be_reentrant(0);
  }
}

__attribute__((visibility("default"))) void
fil_new_line_number(uint16_t line_number) {
  if (!am_i_reentrant()) {
    set_will_i_be_reentrant(1);
    pymemprofile_new_line_number(line_number);
    set_will_i_be_reentrant(0);
  }
}

__attribute__((visibility("default"))) void
fil_reset(const char *default_path) {
  if (!am_i_reentrant()) {
    set_will_i_be_reentrant(1);
    pymemprofile_reset(default_path);
    set_will_i_be_reentrant(0);
  }
}

__attribute__((visibility("default"))) void
fil_dump_peak_to_flamegraph(const char *path) {
  // This maybe called after we're done, when will_i_be_reentrant is permanently
  // set to 1, or might be called mid-way through code run. Either way we want
  // to prevent reentrant malloc() calls, but we want to run regardless.
  int current_reentrant_status = am_i_reentrant();
  set_will_i_be_reentrant(1);
  pymemprofile_dump_peak_to_flamegraph(path);
  set_will_i_be_reentrant(current_reentrant_status);
}

static void add_allocation(size_t address, size_t size) {
  uint16_t line_number = 0;
  PyFrameObject *f = current_frame;
  if (f != NULL) {
    line_number = PyCode_Addr2Line(f->f_code, f->f_lasti);
  }
  pymemprofile_add_allocation(address, size, line_number);
}

static void add_anon_mmap(size_t address, size_t size) {
  uint16_t line_number = 0;
  PyFrameObject *f = current_frame;
  if (f != NULL) {
    line_number = PyCode_Addr2Line(f->f_code, f->f_lasti);
  }
  pymemprofile_add_anon_mmap(address, size, line_number);
}

// On Linux, before the shared library is initialized it's not possible to call
// malloc() and friends, because the function pointers haven't been loaded with
// dlsym(). So we need a fallback. On macOS we can just use the function
// directly, because we're not publishing our own malloc() symbol so we don't
// get infinite recursion, we can just call the original function.
static void *malloc_fallback(size_t size) {
#ifdef __APPLE__
  return malloc(size);
#else
  // We can't use mmap() libc call, because we override it, so use the syscall
  // directly:
  return (void *)syscall(SYS_mmap, NULL, size, PROT_READ | PROT_WRITE,
                         MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
#endif
}

// Override memory-allocation functions:
__attribute__((visibility("default"))) void *
SYMBOL_PREFIX(malloc)(size_t size) {
  if (unlikely(!initialized)) {
    return malloc_fallback(size);
  }
  void *result = underlying_real_malloc(size);
  if (!am_i_reentrant() && initialized) {
    set_will_i_be_reentrant(1);
    add_allocation((size_t)result, size);
    set_will_i_be_reentrant(0);
  }
  return result;
}

__attribute__((visibility("default"))) void *
SYMBOL_PREFIX(calloc)(size_t nmemb, size_t size) {
  if (unlikely(!initialized)) {
    return malloc_fallback(nmemb * size);
  }
  void *result = underlying_real_calloc(nmemb, size);
  size_t allocated = nmemb * size;
  if (!am_i_reentrant() && initialized) {
    set_will_i_be_reentrant(1);
    add_allocation((size_t)result, allocated);
    set_will_i_be_reentrant(0);
  }
  return result;
}

__attribute__((visibility("default"))) void *
SYMBOL_PREFIX(realloc)(void *addr, size_t size) {
  if (unlikely(!initialized)) {
#ifdef __APPLE__
    return realloc(addr, size);
#else
    void *result = malloc_fallback(size);
    if (addr != NULL) {
      // Why someone should realloc() with null pointer I don't know.
      // But they sometimes do.
      memcpy(result, addr, size);
    }
    return result;
#endif
  }
  void *result = underlying_real_realloc(addr, size);
  if (!am_i_reentrant() && initialized) {
    set_will_i_be_reentrant(1);
    // Sometimes you'll get same address, so if we did remove first and then
    // added, it would remove the entry erroneously.
    pymemprofile_free_allocation((size_t)addr);
    add_allocation((size_t)result, size);
    set_will_i_be_reentrant(0);
  }
  return result;
}

__attribute__((visibility("default"))) void SYMBOL_PREFIX(free)(void *addr) {
  if (unlikely(!initialized)) {
#ifdef __APPLE__
    free(addr);
#else
// We're going to leak a little memory, but, such is life...
#endif
    return;
  }
  underlying_real_free(addr);
  if (!am_i_reentrant()) {
    set_will_i_be_reentrant(1);
    pymemprofile_free_allocation((size_t)addr);
    set_will_i_be_reentrant(0);
  }
}

__attribute__((visibility("default"))) void *
SYMBOL_PREFIX(mmap)(void *addr, size_t length, int prot, int flags, int fd,
                    off_t offset) {
  if (unlikely(!initialized)) {
#ifdef __APPLE__
    return mmap(addr, length, prot, flags, fd, offset);
#else
    return (void*) syscall(SYS_mmap, addr, length, prot, flags, fd, offset);
#endif
  }

  void *result = underlying_real_mmap(addr, length, prot, flags, fd, offset);

  // For now we only track anonymous mmap()s:
  if ((flags & MAP_ANONYMOUS) && !am_i_reentrant()) {
    set_will_i_be_reentrant(1);
    add_anon_mmap((size_t)result, length);
    set_will_i_be_reentrant(0);
  }
  return result;
}

__attribute__((visibility("default"))) int
SYMBOL_PREFIX(munmap)(void *addr, size_t length) {
  if (unlikely(!initialized)) {
#ifdef __APPLE__
    return munmap(addr, length);
#else
    return syscall(SYS_munmap, addr, length);
#endif
  }

  int result = underlying_real_munmap(addr, length);
  if (!am_i_reentrant()) {
    set_will_i_be_reentrant(1);
    // TODO handle length
    pymemprofile_free_anon_mmap(result, length);
    set_will_i_be_reentrant(0);
  }
  return result;
}

#ifdef __APPLE__
DYLD_INTERPOSE(SYMBOL_PREFIX(malloc), malloc)
DYLD_INTERPOSE(SYMBOL_PREFIX(calloc), calloc)
DYLD_INTERPOSE(SYMBOL_PREFIX(realloc), realloc)
DYLD_INTERPOSE(SYMBOL_PREFIX(free), free)
DYLD_INTERPOSE(SYMBOL_PREFIX(mmap), mmap)
DYLD_INTERPOSE(SYMBOL_PREFIX(munmap), munmap)
#endif

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
  set_will_i_be_reentrant(1);
}
