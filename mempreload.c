#define _GNU_SOURCE

#include <stdlib.h>
#include <stdio.h>
#include <dlfcn.h>
#include <malloc.h>
#include <sys/mman.h>

// Underlying mmap() API:
static void* (*underlying_real_mmap)(void *addr, size_t length, int prot, int flags, int fd, off_t offset) = 0;

// The internal API we're notifying of allocations:
static void (*update_memory_usage)(void) = 0;

// Note whether we've been initialized yet or not:
static int initialized = 0;

static int will_i_be_reentrant = 0;

static void __attribute__((constructor)) constructor() {
  if (initialized) {
    return;
  }
  void* lib = dlopen("target/debug/libpymemprofile_api.so", RTLD_LAZY | RTLD_DEEPBIND);
  if (!lib) {
    fprintf(stderr, "Couldn't load libpymemprofile_api.so library: %s\n", dlerror());
    exit(1);
  }
  update_memory_usage = dlsym(lib, "pymemprofile_update_memory_usage");
  if (!update_memory_usage) {
    fprintf(stderr, "Couldn't load pymemprofile API function: %s\n", dlerror());
    exit(1);
  }
  underlying_real_mmap = dlsym(RTLD_NEXT, "mmap");
  if (!underlying_real_mmap) {
    fprintf(stderr, "Couldn't load mmap(): %s\n", dlerror());
    exit(1);
  }
  initialized = 1;
}

extern void *__libc_malloc(size_t size);
extern void *__libc_calloc(size_t nmemb, size_t size);

// Override memory-allocation functions:
__attribute__ ((visibility("default"))) void* malloc(size_t size)  {
  void* result = __libc_malloc(size);
  if (!will_i_be_reentrant && initialized) {
    will_i_be_reentrant = 1;
    update_memory_usage();
    will_i_be_reentrant = 0;
  }
  return result;
}

__attribute__ ((visibility("default"))) void* calloc(size_t nmemb, size_t size) {
  void* result = __libc_calloc(nmemb, size);
  if (!will_i_be_reentrant && initialized) {
    will_i_be_reentrant = 1;
    update_memory_usage();
    will_i_be_reentrant = 0;
  }
  return result;
}

__attribute__ ((visibility("default"))) void* mmap(void *addr, size_t length, int prot, int flags, int fd, off_t offset) {
  if (!initialized) {
    constructor();
  }
  void* result = underlying_real_mmap(addr, length, prot, flags, fd, offset);
  fprintf(stdout, "MMAP!\n");
  if ((flags & (MAP_PRIVATE | MAP_ANONYMOUS)) && !will_i_be_reentrant && initialized) {
    will_i_be_reentrant = 1;
    update_memory_usage();
    will_i_be_reentrant = 0;
  }
  return result;
}
