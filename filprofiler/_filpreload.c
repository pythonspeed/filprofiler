#define _GNU_SOURCE

#include <dlfcn.h>
#include <malloc.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/mman.h>

// Underlying APIs we're wrapping:
static void *(*underlying_real_mmap)(void *addr, size_t length, int prot,
                                     int flags, int fd, off_t offset) = 0;
static void (*underlying_real_free)(void *addr) = 0;

// The internal API we're notifying of allocations:
static void (*add_allocation_hook)(size_t address, size_t length) = 0;
static void (*free_allocation_hook)(size_t address) = 0;

// Note whether we've been initialized yet or not:
static int initialized = 0;

static _Thread_local int will_i_be_reentrant = 0;

static void __attribute__((constructor)) constructor() {
  if (initialized) {
    return;
  }
  unsetenv("LD_PRELOAD");
  if (sizeof((void *)0) != sizeof((size_t)0)) {
    fprintf(stderr, "BUG: expected size of size_t and void* to be the same.\n");
    exit(1);
  }
  void *lib =
      dlopen(getenv("FIL_API_LIBRARY"), RTLD_NOW | RTLD_DEEPBIND | RTLD_GLOBAL);
  if (!lib) {
    fprintf(stderr, "Couldn't load libpymemprofile_api.so library: %s\n",
            dlerror());
    exit(1);
  }
  add_allocation_hook = dlsym(lib, "pymemprofile_add_allocation");
  if (!add_allocation_hook) {
    fprintf(stderr, "Couldn't load pymemprofile API function: %s\n", dlerror());
    exit(1);
  }
  free_allocation_hook = dlsym(lib, "pymemprofile_free_allocation");
  if (!free_allocation_hook) {
    fprintf(stderr, "Couldn't load pymemprofile API function: %s\n", dlerror());
    exit(1);
  }
  underlying_real_mmap = dlsym(RTLD_NEXT, "mmap");
  if (!underlying_real_mmap) {
    fprintf(stderr, "Couldn't load mmap(): %s\n", dlerror());
    exit(1);
  }
  underlying_real_free = dlsym(RTLD_NEXT, "free");
  if (!underlying_real_free) {
    fprintf(stderr, "Couldn't load free(): %s\n", dlerror());
    exit(1);
  }
  initialized = 1;
}

extern void *__libc_malloc(size_t size);
extern void *__libc_calloc(size_t nmemb, size_t size);
extern void pymemprofile_start_call(const char *filename, const char *funcname);
extern void pymemprofile_finish_call();
extern void pymemprofile_reset();
extern void pymemprofile_dump_peak_to_flamegraph(const char* path);

__attribute__((visibility("default"))) void
fil_start_call(const char *filename, const char *funcname) {
  if (!will_i_be_reentrant) {
    will_i_be_reentrant = 1;
    pymemprofile_start_call(filename, funcname);
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

__attribute__((visibility("default"))) void fil_reset() {
  if (!will_i_be_reentrant) {
    will_i_be_reentrant = 1;
    pymemprofile_reset();
    will_i_be_reentrant = 0;
  }
}

__attribute__((visibility("default"))) void fil_dump_peak_to_flamegraph(const char* path) {
  if (!will_i_be_reentrant) {
    will_i_be_reentrant = 1;
    pymemprofile_dump_peak_to_flamegraph(path);
    will_i_be_reentrant = 0;
  }
}

// Override memory-allocation functions:
__attribute__((visibility("default"))) void *malloc(size_t size) {
  void *result = __libc_malloc(size);
  if (!will_i_be_reentrant && initialized) {
    will_i_be_reentrant = 1;
    add_allocation_hook((size_t)result, size);
    will_i_be_reentrant = 0;
  }
  return result;
}

__attribute__((visibility("default"))) void *calloc(size_t nmemb, size_t size) {
  void *result = __libc_calloc(nmemb, size);
  size_t allocated = nmemb * size;
  if (!will_i_be_reentrant && initialized) {
    will_i_be_reentrant = 1;
    add_allocation_hook((size_t)result, allocated);
    will_i_be_reentrant = 0;
  }
  return result;
}

__attribute__((visibility("default"))) void free(void *addr) {
  if (!initialized) {
    // Well, we're going to leak a little memory, but, such is life...
    return;
  }
  underlying_real_free(addr);
  if (!will_i_be_reentrant) {
    will_i_be_reentrant = 1;
    free_allocation_hook((size_t)addr);
    will_i_be_reentrant = 0;
  }
}

/*
__attribute__ ((visibility("default"))) void* mmap(void *addr, size_t length,
int prot, int flags, int fd, off_t offset) { if (!initialized) { constructor();
  }
  void* result = underlying_real_mmap(addr, length, prot, flags, fd, offset);
  fprintf(stdout, "MMAP!\n");
  if ((flags & (MAP_PRIVATE | MAP_ANONYMOUS)) && !will_i_be_reentrant &&
initialized) { will_i_be_reentrant = 1; update_memory_usage();
    will_i_be_reentrant = 0;
  }
  return result;
}
*/
