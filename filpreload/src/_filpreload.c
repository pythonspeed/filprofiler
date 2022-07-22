#include "Python.h"
#include "code.h"
#include "object.h"
#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif
#include "frameobject.h"
#include <dlfcn.h>
#include <pthread.h>
#include <stdatomic.h>
#include <stdint.h>
#include <stdio.h>
#include <sys/mman.h>
#include <sys/types.h>
#include <unistd.h>
#include <stdbool.h>
#include <errno.h>

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
#define REAL_IMPL(func) __libc_##func
#endif

#define likely(x) __builtin_expect(!!(x), 1)
#define unlikely(x) __builtin_expect(!!(x), 0)

// Underlying APIs we're wrapping:
static void *(*underlying_real_mmap)(void *addr, size_t length, int prot,
                                     int flags, int fd, off_t offset) = 0;
static int (*underlying_real_pthread_create)(pthread_t *thread,
                                             const pthread_attr_t *attr,
                                             void *(*start_routine)(void *),
                                             void *arg) = 0;
static pid_t (*underlying_real_fork)(void) = 0;

#ifdef __linux__
extern void *__libc_malloc(size_t length);
extern void *__libc_calloc(size_t nmemb, size_t length);
extern void *__libc_realloc(void *addr, size_t length);
extern void __libc_free(void *addr);
extern void *__libc_memalign(size_t alignment, size_t size);
#endif

// Note whether we've been initialized yet or not:
static int initialized = 0;

// Note whether we're currently tracking allocations. Jupyter users might turn
// this on and then off, for example, whereas full process profiling will have
// this on from start until finish.
static _Atomic int tracking_allocations = ATOMIC_VAR_INIT(0);

// ID of Python code object extra data:
static Py_ssize_t extra_code_index = -1;

#ifdef __APPLE__
#include "interpose.h"
static pthread_key_t will_i_be_reentrant;
static pthread_once_t will_i_be_reentrant_once = PTHREAD_ONCE_INIT;

static void make_pthread_key() {
  pthread_key_create(&will_i_be_reentrant, (void *)0);
}

// 0 means not reentrant, other values means it is.
static inline uint64_t am_i_reentrant() {
  (void)pthread_once(&will_i_be_reentrant_once, make_pthread_key);
  return (int)pthread_getspecific(will_i_be_reentrant);
}

static inline void increment_reentrancy() {
  int current = (int) pthread_getspecific(will_i_be_reentrant);
  pthread_setspecific(will_i_be_reentrant, (void *)(current + 1));
}

static inline void decrement_reentrancy() {
  int current = (int)pthread_getspecific(will_i_be_reentrant);
  pthread_setspecific(will_i_be_reentrant, (void *)(current - 1));
}

#elif __linux__
#include <sys/syscall.h>
static _Thread_local uint64_t will_i_be_reentrant = 0;

// 0 means not reentrant, other values means it is.
static inline uint64_t am_i_reentrant() { return will_i_be_reentrant; }

static inline void increment_reentrancy() { will_i_be_reentrant += 1; }
static inline void decrement_reentrancy() { will_i_be_reentrant -= 1; }
#endif

// Versions for calling from Rust
void fil_increment_reentrancy() {
  increment_reentrancy();
}

void fil_decrement_reentrancy() {
  decrement_reentrancy();
}

// Return whether to pass malloc() etc. to Rust tracking code.
// Will be true if all conditions are true:
//
// 1. The shared library constructor is initialized; always true after that.
// 2. Allocations are being tracked.
// 3. This isn't a reentrant call: we don't want to track memory allocations
//    triggered by the Rust tracking code, as that will result in infinite
//    recursion.
static inline int should_track_memory() {
  return (likely(initialized) && atomic_load_explicit(&tracking_allocations, memory_order_acquire) && !am_i_reentrant());
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

// Implemented in the Rust library:
extern uint64_t pymemprofile_add_function_location(const char* filename, size_t filename_length, const char* function_name,
                                                   size_t function_length);
extern void pymemprofile_start_call(uint16_t parent_line_number,
                                    uint64_t function_id,
                                    uint16_t line_number);
extern void pymemprofile_finish_call();
extern void pymemprofile_new_line_number(uint16_t line_number);
extern void pymemprofile_reset(const char *path);
extern void pymemprofile_start_tracking();
extern void pymemprofile_stop_tracking();
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

static void __attribute__((constructor)) constructor() {
  if (initialized) {
    return;
  }

  if (sizeof((void *)0) != sizeof((size_t)0)) {
    fprintf(stderr, "BUG: expected size of size_t and void* to be the same.\n");
    exit(1);
  }

#ifdef __APPLE__
  // On macOS Monterey dlsym() is pointing at symbols in current shared library
  // for some reason, so just use DYLD_INTERPOSE.
  underlying_real_mmap = REAL_IMPL(mmap);
  underlying_real_pthread_create = REAL_IMPL(pthread_create);
  underlying_real_fork = REAL_IMPL(fork);
#else
  underlying_real_mmap = dlsym(RTLD_NEXT, "mmap");
  if (!underlying_real_mmap) {
    fprintf(stderr, "Couldn't load mmap(): %s\n", dlerror());
    exit(1);
  }
  underlying_real_pthread_create = dlsym(RTLD_NEXT, "pthread_create");
  if (!underlying_real_pthread_create) {
    fprintf(stderr, "Couldn't load pthread_create(): %s\n", dlerror());
    exit(1);
  }
  underlying_real_fork = dlsym(RTLD_NEXT, "fork");
  if (!underlying_real_fork) {
    fprintf(stderr, "Couldn't load fork(): %s\n", dlerror());
    exit(1);
  }
#endif
  // Initialize Rust static state before we start doing any calls via malloc(),
  // to ensure we don't get unpleasant reentrancy issues.
  pymemprofile_reset("/tmp");

  // Drop LD_PRELOAD so that Linux subprocesses don't have this preloaded.
  unsetenv("LD_PRELOAD");

  // Enabling this breaks things. Don't trust CI being green, check this
  // manually (see https://github.com/pythonspeed/filprofiler/issues/137). So
  // instead we do it in fork(), post-constructor, where apparently it
  // is fine to do.
  // unsetenv("DYLD_INSERT_LIBRARIES");

  initialized = 1;
}

static void start_call(uint64_t function_id, uint16_t line_number) {
  if (should_track_memory()) {
    increment_reentrancy();
    uint16_t parent_line_number = 0;
    if (current_frame != NULL && current_frame->f_back != NULL) {
      PyFrameObject *f = current_frame->f_back;
      parent_line_number = PyFrame_GetLineNumber(f);
    }
    pymemprofile_start_call(parent_line_number, function_id, line_number);
    decrement_reentrancy();
  }
}

static void finish_call() {
  if (should_track_memory()) {
    increment_reentrancy();
    pymemprofile_finish_call();
    decrement_reentrancy();
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
      We want an efficient identifier for filename+fuction name. So we register
      the function + filename with some Rust code that gives back its ID, and
      then store the ID. Due to bad API design, value 0 indicates "no result",
      so we actually store the result + 1.
    */
    uint64_t function_id = 0;
    assert(extra_code_index != -1);
    _PyCode_GetExtra((PyObject *)frame->f_code, extra_code_index,
                     (void **)&function_id);
    if (function_id == 0) {
      Py_ssize_t filename_length, function_length;
      const char* filename = PyUnicode_AsUTF8AndSize(frame->f_code->co_filename,
                                                     &filename_length);
      const char* function_name = PyUnicode_AsUTF8AndSize(frame->f_code->co_name,
                                                          &function_length);
      increment_reentrancy();
      function_id = pymemprofile_add_function_location(filename, (uint64_t)filename_length, function_name, (uint64_t)function_length);
      decrement_reentrancy();
      _PyCode_SetExtra((PyObject *)frame->f_code, extra_code_index,
                       (void *)function_id + 1);
    } else {
      function_id -= 1;
    }
    start_call(function_id, frame->f_lineno);
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
fil_start_tracking() {
  atomic_store_explicit(&tracking_allocations, 1, memory_order_release);
}

/// Clear previous allocations;
__attribute__((visibility("default"))) void
fil_reset(const char *default_path) {
  increment_reentrancy();
  pymemprofile_reset(default_path);
  decrement_reentrancy();
}

/// End memory tracing.
__attribute__((visibility("default"))) void fil_stop_tracking() {
  atomic_store_explicit(&tracking_allocations, 0, memory_order_release);
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
  // We want to prevent reentrant malloc() calls, but we want to run regardless
  // of whether this particular call is reentrant.
  increment_reentrancy();
  pymemprofile_dump_peak_to_flamegraph(path);
  decrement_reentrancy();
}

// *** End APIs called by Python ***
static void add_allocation(size_t address, size_t size) {
  uint16_t line_number = 0;
  PyFrameObject *f = current_frame;
  if (f != NULL) {
    line_number = PyFrame_GetLineNumber(f);
  }
  pymemprofile_add_allocation(address, size, line_number);
}

static void add_anon_mmap(size_t address, size_t size) {
  uint16_t line_number = 0;
  PyFrameObject *f = current_frame;
  if (f != NULL) {
    line_number = PyFrame_GetLineNumber(f);
  }
  pymemprofile_add_anon_mmap(address, size, line_number);
}

// Disable memory tracking after fork() in the child.
__attribute__((visibility("default"))) pid_t SYMBOL_PREFIX(fork)(void) {
  // Make sure subprocesses on macOS don't preload this:
  increment_reentrancy();
  unsetenv("DYLD_INSERT_LIBRARIES");
  decrement_reentrancy();

  static int already_printed = 0;
  if (atomic_load_explicit(&tracking_allocations, memory_order_acquire) && !already_printed) {
    fprintf(stderr, "=fil-profile= WARNING: Fil does not (yet) support tracking memory in subprocesses.\n");
    already_printed = 1;
  }
  pid_t result = underlying_real_fork();
  if (result == 0) {
    // We're the child.
    // Change status. This is actually done in Python code too
    // (filprofiler/__init__.py), so os.environ stays in sync. Doing it in only
    // C or only Python doesn't seem to work, need both for some reason.
    setenv("__FIL_STATUS", "subprocess", 1);
    // Clear any memory if we're in icky fork()-without-exec() mode.
    fil_stop_tracking();
  }
  return result;
}

// Override memory-allocation functions:
__attribute__((visibility("default"))) void *
SYMBOL_PREFIX(malloc)(size_t size) {
  increment_reentrancy();
  void *result = REAL_IMPL(malloc)(size);
  decrement_reentrancy();
  if (should_track_memory()) {
    increment_reentrancy();
    add_allocation((size_t)result, size);
    decrement_reentrancy();
  }
  return result;
}

__attribute__((visibility("default"))) void *
SYMBOL_PREFIX(calloc)(size_t nmemb, size_t size) {
  increment_reentrancy();
  void *result = REAL_IMPL(calloc)(nmemb, size);
  decrement_reentrancy();
  size_t allocated = nmemb * size;
  if (should_track_memory()) {
    increment_reentrancy();
    add_allocation((size_t)result, allocated);
    decrement_reentrancy();
  }
  return result;
}

__attribute__((visibility("default"))) void *
SYMBOL_PREFIX(realloc)(void *addr, size_t size) {
  // We do removal bookkeeping first. Otherwise, as soon as the freeing happens
  // another thread may allocate the same address, leading to a race condition
  // in the bookkeeping metadata.
  //
  // If realloc() fails due to lack of memory, this will result in memory still
  // existing but Fil thinking it's gone. However, at that point Fil will then
  // exit with OOM report, so... not the end of the world, and unlikely in
  // practice.
  if (should_track_memory() && ((size_t)addr != 0)) {
    increment_reentrancy();
    // Sometimes you'll get same address, so if we did add first and then
    // removed, it would remove the entry erroneously.
    pymemprofile_free_allocation((size_t)addr);
    decrement_reentrancy();
  }
  increment_reentrancy();
  void *result = REAL_IMPL(realloc)(addr, size);
  decrement_reentrancy();
  if (should_track_memory()) {
    increment_reentrancy();
    add_allocation((size_t)result, size);
    decrement_reentrancy();
  }
  return result;
}

#if __linux__
int __libc_posix_memalign(void **memptr, size_t alignment, size_t size) {
  void* result = __libc_memalign(alignment, size);
  if ((result == NULL) && size != 0) {
    return ENOMEM;
  } else {
    *memptr = result;
    return 0;
  }
}

void* __libc_aligned_alloc(size_t alignment, size_t size) {
  return __libc_memalign(alignment, size);
}
#endif

__attribute__((visibility("default"))) int SYMBOL_PREFIX(posix_memalign)(
        void **memptr, size_t alignment, size_t size) {
  increment_reentrancy();
  int result = REAL_IMPL(posix_memalign)(memptr, alignment, size);
  decrement_reentrancy();
  if (!result && should_track_memory()) {
    increment_reentrancy();
    add_allocation((size_t)*memptr, size);
    decrement_reentrancy();
  }
  return result;
}

__attribute__((visibility("default"))) void SYMBOL_PREFIX(free)(void *addr) {
  // We do bookkeeping first. Otherwise, as soon as the free() happens another
  // thread may allocate the same address, leading to a race condition in the
  // bookkeeping metadata.
  if (should_track_memory()) {
    increment_reentrancy();
    pymemprofile_free_allocation((size_t)addr);
    decrement_reentrancy();
  }
  increment_reentrancy();
  REAL_IMPL(free)(addr);
  decrement_reentrancy();
}

// On Linux this is exposed via --wrap, to get both mmap() and mmap64() without
// fighting the fact that glibc #defines mmap as mmap64 sometimes...
__attribute__((visibility("default"))) void *
fil_mmap_impl(void *addr, size_t length, int prot, int flags, int fd,
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
    increment_reentrancy();
    add_anon_mmap((size_t)result, length);
    decrement_reentrancy();
  }
  return result;
}

#ifdef __APPLE__
__attribute__((visibility("default"))) void *
SYMBOL_PREFIX(mmap)(void *addr, size_t length, int prot, int flags, int fd,
                    off_t offset) {
  return fil_mmap_impl(addr, length, prot, flags, fd, offset);
}
#endif

// Old glibc that Conda uses defines aligned_alloc() using inline that doesn't
// match this signature, which messes up the SYMBOL_PREFIX() stuff on Linux. So,
// we do reimplemented_aligned_alloc, the name macOS technique uses, and then
// rely on symbol alias (see --defsym in setup.py) to fix it.
//
// On macOS, aligned_alloc is only in macOS 10.15 or later, we need to define
// it.
#ifdef __APPLE__
void *aligned_alloc(size_t alignment, size_t size);
#endif

__attribute__((visibility("default"))) void *
reimplemented_aligned_alloc(size_t alignment, size_t size) {
  increment_reentrancy();
  void *result = REAL_IMPL(aligned_alloc)(alignment, size);
  decrement_reentrancy();

  if (should_track_memory()) {
    increment_reentrancy();
    add_allocation((size_t)result, size);
    decrement_reentrancy();
  }
  return result;
}

// Argument for wrapper_pthread_start().
struct NewThreadArgs {
  void *callstack;
  void *(*start_routine)(void *);
  void *arg;
};

// Called as starting function for new threads. Sets callstack, then calls the
// real starting function.
static void *wrapper_pthread_start(void *nta) {
  struct NewThreadArgs *args = (struct NewThreadArgs *)nta;
  increment_reentrancy();
  pymemprofile_set_current_callstack(args->callstack);
  decrement_reentrancy();
  void *(*start_routine)(void *) = args->start_routine;
  void *arg = args->arg;
  REAL_IMPL(free)(args);

  // Run the underlying thread code:
  return start_routine(arg);
}

// Override pthread_create so that new threads copy the current thread's Python
// callstack.
__attribute__((visibility("default"))) int
SYMBOL_PREFIX(pthread_create)(pthread_t *thread, const pthread_attr_t *attr,
                              void *(*start_routine)(void *), void *arg) {
  if (!likely(initialized) || am_i_reentrant()) {
    return underlying_real_pthread_create(thread, attr, start_routine, arg);
  }
  struct NewThreadArgs *wrapper_args =
      REAL_IMPL(malloc)(sizeof(struct NewThreadArgs));
  wrapper_args->callstack = pymemprofile_get_current_callstack();
  wrapper_args->start_routine = start_routine;
  wrapper_args->arg = arg;
  int result = underlying_real_pthread_create(
      thread, attr, &wrapper_pthread_start, (void *)wrapper_args);
  return result;
}

#ifdef __APPLE__
extern int reimplemented_munmap(void *addr, size_t length);
DYLD_INTERPOSE(SYMBOL_PREFIX(malloc), malloc)
DYLD_INTERPOSE(SYMBOL_PREFIX(calloc), calloc)
DYLD_INTERPOSE(SYMBOL_PREFIX(realloc), realloc)
DYLD_INTERPOSE(SYMBOL_PREFIX(free), free)
DYLD_INTERPOSE(SYMBOL_PREFIX(mmap), mmap)
DYLD_INTERPOSE(SYMBOL_PREFIX(munmap), munmap)
DYLD_INTERPOSE(SYMBOL_PREFIX(aligned_alloc), aligned_alloc)
DYLD_INTERPOSE(SYMBOL_PREFIX(posix_memalign), posix_memalign)
DYLD_INTERPOSE(SYMBOL_PREFIX(pthread_create), pthread_create)
DYLD_INTERPOSE(SYMBOL_PREFIX(fork), fork)
#endif


/////// RUST API ///////////

// Call a function in non-reentrant way. For use from Rust code.
void call_if_tracking(void (*f)(void *), void *user_data) {
  if (should_track_memory()) {
    increment_reentrancy();
    f(user_data);
    decrement_reentrancy();
  }
}

// Expose initialized to Rust()
int is_initialized() {
  return initialized;
}
