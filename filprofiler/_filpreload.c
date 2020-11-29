#include "Python.h"
#include "code.h"
#include "object.h"
#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif
#include "frameobject.h"
#include <dlfcn.h>
#include <pthread.h>
#include <stdint.h>
#include <stdio.h>
#include <sys/mman.h>

// Macro to create the publicly exposed symbol:
#ifdef __APPLE__
#define SYMBOL_PREFIX(func) reimplemented_##func
#else
#define SYMBOL_PREFIX(func) func
#endif

// Macro to get the underlying function being wrapped:
#ifdef __APPLE__
#define REAL_IMPL(func) func
#elif __linux__
#define REAL_IMPL(func) _rjem_##func
#endif

#define likely(x) __builtin_expect(!!(x), 1)
#define unlikely(x) __builtin_expect(!!(x), 0)

// Underlying APIs we're wrapping:
static void *(*underlying_real_mmap)(void *addr, size_t length, int prot,
                                     int flags, int fd, off_t offset) = 0;
static int (*underlying_real_munmap)(void *addr, size_t length) = 0;
static int (*underlying_real_pthread_create)(pthread_t *thread,
                                             const pthread_attr_t *attr,
                                             void *(*start_routine)(void *),
                                             void *arg) = 0;

// Used on Linux to implement these APIs:
extern void *_rjem_malloc(size_t length);
extern void *_rjem_calloc(size_t nmemb, size_t length);
extern void *_rjem_realloc(void *addr, size_t length);
extern void _rjem_free(void *addr);
extern void *_rjem_aligned_alloc(size_t alignment, size_t size);
extern size_t _rjem_malloc_usable_size(void *ptr);
extern int _rjem_posix_memalign(void **memptr, size_t alignment, size_t size);

// Note whether we've been initialized yet or not:
static int initialized = 0;

// Note whether we're currently tracking allocations. Jupyter users might turn
// this on and then off, for example, whereas full process profiling will have
// this on from start until finish.
static int tracking_allocations = 0;

// ID of Python code object extra data:
static Py_ssize_t extra_code_index = -1;

#ifdef __APPLE__
#include "interpose.h"
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
#elif __linux__
#include <sys/syscall.h>
static _Thread_local int will_i_be_reentrant = 0;

static inline int am_i_reentrant() { return will_i_be_reentrant; }

static inline void set_will_i_be_reentrant(int i) { will_i_be_reentrant = i; }
#endif

// Return whether to pass malloc() etc. to Rust tracking code.
// Will be true if all conditions are true:
//
// 1. The shared library constructor is initialized; always true after that.
// 2. Allocations are being tracked.
// 3. This isn't a reentrant call: we don't want to track memory allocations
//    triggered by the Rust tracking code, as that will result in infinite
//    recursion.
static inline int should_track_memory() {
  return (likely(initialized) && tracking_allocations && !am_i_reentrant());
}

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

#ifdef __linux__
  // Ensure jemalloc is initialized as early as possible. If jemalloc is
  // initialized via mmap() -> Rust triggering allocation, it deadlocks because
  // jemalloc uses mmap to get more memory!
  _rjem_malloc(1);
#endif

  if (sizeof((void *)0) != sizeof((size_t)0)) {
    fprintf(stderr, "BUG: expected size of size_t and void* to be the same.\n");
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
  underlying_real_pthread_create = dlsym(RTLD_NEXT, "pthread_create");
  if (!underlying_real_pthread_create) {
    fprintf(stderr, "Couldn't load pthread_create(): %s\n", dlerror());
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
extern void *pymemprofile_get_current_callstack();
extern void pymemprofile_set_current_callstack(void *callstack);
extern void pymemprofile_clear_current_callstack();

static void start_call(struct FunctionLocation *loc, uint16_t line_number) {
  if (should_track_memory()) {
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

static void finish_call() {
  if (should_track_memory()) {
    set_will_i_be_reentrant(1);
    pymemprofile_finish_call();
    set_will_i_be_reentrant(0);
  }
}

/// Callback functions for the Python tracing API (PyEval_SetProfile).
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
      loc = REAL_IMPL(malloc)(sizeof(struct FunctionLocation));
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

// *** APIs called by Python ***

/// Called after Python gets going, allowing us to call some necessary Python
/// APIs.
__attribute__((visibility("default"))) void fil_initialize_from_python() {
  extra_code_index = _PyEval_RequestCodeExtraIndex(NULL);
}

/// Start memory tracing.
__attribute__((visibility("default"))) void
fil_reset(const char *default_path) {
  tracking_allocations = 1;
  set_will_i_be_reentrant(1);
  pymemprofile_reset(default_path);
  set_will_i_be_reentrant(0);
}

/// End memory tracing.
__attribute__((visibility("default"))) void fil_shutting_down() {
  tracking_allocations = 0;
}

/// Register the C level Python tracer for the current thread.
__attribute__((visibility("default"))) void register_fil_tracer() {
  // C threads inherit their callstack from the creating Python thread. That's
  // fine. However, if a tracer is being registered, that means this is not a
  // pure C thread, it's a new Python thread with its own callstack.
  pymemprofile_clear_current_callstack();
  // We use 123 as a marker object for tests.
  PyEval_SetProfile(fil_tracer, PyLong_FromLong(123));
}

/// Dump the current peak memory usage to disk.
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

// *** End APIs called by Python ***

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

// Override memory-allocation functions:
__attribute__((visibility("default"))) void *
SYMBOL_PREFIX(malloc)(size_t size) {
  void *result = REAL_IMPL(malloc)(size);
  if (should_track_memory()) {
    set_will_i_be_reentrant(1);
    add_allocation((size_t)result, size);
    set_will_i_be_reentrant(0);
  }
  return result;
}

__attribute__((visibility("default"))) void *
SYMBOL_PREFIX(calloc)(size_t nmemb, size_t size) {
  void *result = REAL_IMPL(calloc)(nmemb, size);
  size_t allocated = nmemb * size;
  if (should_track_memory()) {
    set_will_i_be_reentrant(1);
    add_allocation((size_t)result, allocated);
    set_will_i_be_reentrant(0);
  }
  return result;
}

__attribute__((visibility("default"))) void *
SYMBOL_PREFIX(realloc)(void *addr, size_t size) {
  void *result = REAL_IMPL(realloc)(addr, size);
  if (should_track_memory()) {
    set_will_i_be_reentrant(1);
    // Sometimes you'll get same address, so if we did add first and then
    // removed, it would remove the entry erroneously.
    pymemprofile_free_allocation((size_t)addr);
    add_allocation((size_t)result, size);
    set_will_i_be_reentrant(0);
  }
  return result;
}

__attribute__((visibility("default"))) int
SYMBOL_PREFIX(posix_memalign)(void **memptr, size_t alignment, size_t size) {
  int result = REAL_IMPL(posix_memalign)(memptr, alignment, size);
  if (!result && should_track_memory()) {
    set_will_i_be_reentrant(1);
    add_allocation((size_t)*memptr, size);
    set_will_i_be_reentrant(0);
  }
  return result;
}

__attribute__((visibility("default"))) void SYMBOL_PREFIX(free)(void *addr) {
  REAL_IMPL(free)(addr);
  if (should_track_memory()) {
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
    return (void *)syscall(SYS_mmap, addr, length, prot, flags, fd, offset);
#endif
  }

  void *result = underlying_real_mmap(addr, length, prot, flags, fd, offset);

  // For now we only track anonymous mmap()s:
  if (result != MAP_FAILED && (flags & MAP_ANONYMOUS) &&
      should_track_memory()) {
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
  if (result != -1 && should_track_memory()) {
    set_will_i_be_reentrant(1);
    pymemprofile_free_anon_mmap(result, length);
    set_will_i_be_reentrant(0);
  }
  return result;
}

__attribute__((visibility("default"))) void *
SYMBOL_PREFIX(aligned_alloc)(size_t alignment, size_t size) {
  void *result = REAL_IMPL(aligned_alloc)(alignment, size);

  // For now we only track anonymous mmap()s:
  if (should_track_memory()) {
    set_will_i_be_reentrant(1);
    add_allocation((size_t)result, size);
    set_will_i_be_reentrant(0);
  }
  return result;
}

#ifdef __linux__
// Make sure we expose jemalloc variant of malloc_usable_size(), in case someone
// actually uses it.
__attribute__((visibility("default"))) size_t
SYMBOL_PREFIX(malloc_usable_size)(void *ptr) {
  return REAL_IMPL(malloc_usable_size)(ptr);
}
#endif

// Argument for wrapper_pthread_start().
struct NewThreadArgs {
  void *callstack;
  void *(*start_routine)(void *);
  void *arg;
};

// Called during thread shutdown. Makes sure we don't call back into the Rust
// code, since that uses thread-local storage which will not be valid
// momentarily.
static void thread_shutdown_handler(void *arg) {
  set_will_i_be_reentrant(1);
}

// Called as starting function for new threads. Sets callstack, then calls the
// real starting function.
static void *wrapper_pthread_start(void *nta) {
  struct NewThreadArgs *args = (struct NewThreadArgs *)nta;
  void* result = NULL;
  set_will_i_be_reentrant(1);
  pymemprofile_set_current_callstack(args->callstack);
  set_will_i_be_reentrant(0);
  void *(*start_routine)(void *) = args->start_routine;
  void *arg = args->arg;
  REAL_IMPL(free)(args);

  // Register shutdown handler:
  pthread_cleanup_push(thread_shutdown_handler, NULL);
  // Run the underlying thread code:
  result = start_routine(arg);
  pthread_cleanup_pop(1);
  return result;
}

// Override pthread_create so that new threads copy the current thread's Python
// callstack.
__attribute__((visibility("default"))) int
SYMBOL_PREFIX(pthread_create)(pthread_t *thread, const pthread_attr_t *attr,
                              void *(*start_routine)(void *), void *arg) {
  if (!likely(initialized) || am_i_reentrant()) {
    return underlying_real_pthread_create(thread, attr, start_routine, arg);
  }
  set_will_i_be_reentrant(1);
  struct NewThreadArgs *wrapper_args =
      REAL_IMPL(malloc)(sizeof(struct NewThreadArgs));
  wrapper_args->callstack = pymemprofile_get_current_callstack();
  wrapper_args->start_routine = start_routine;
  wrapper_args->arg = arg;
  int result = underlying_real_pthread_create(
      thread, attr, &wrapper_pthread_start, (void *)wrapper_args);
  set_will_i_be_reentrant(0);
  return result;
}

#ifdef __APPLE__
DYLD_INTERPOSE(SYMBOL_PREFIX(malloc), malloc)
DYLD_INTERPOSE(SYMBOL_PREFIX(calloc), calloc)
DYLD_INTERPOSE(SYMBOL_PREFIX(realloc), realloc)
DYLD_INTERPOSE(SYMBOL_PREFIX(free), free)
DYLD_INTERPOSE(SYMBOL_PREFIX(mmap), mmap)
DYLD_INTERPOSE(SYMBOL_PREFIX(munmap), munmap)
DYLD_INTERPOSE(SYMBOL_PREFIX(aligned_alloc), aligned_alloc)
DYLD_INTERPOSE(SYMBOL_PREFIX(posix_memalign), posix_memalign)
DYLD_INTERPOSE(SYMBOL_PREFIX(pthread_create), pthread_create)
#endif
