#define _GNU_SOURCE

#include <stdlib.h>
#include <stdio.h>
#include <dlfcn.h>
#include <malloc.h>
#include <sys/mman.h>

// Underlying APIs we're wrapping:
static void* (*underlying_real_mmap)(void *addr, size_t length, int prot, int flags, int fd, off_t offset) = 0;
static void (*underlying_real_free)(void* addr) = 0;

// The internal API we're notifying of allocations:
static void (*add_allocation_hook)(size_t address, size_t length) = 0;
static void (*free_allocation_hook)(size_t address) = 0;

// Note whether we've been initialized yet or not:
static int initialized = 0;

// TODO switch to `static __Thread_local` inside function, *probably* stack based?
static int will_i_be_reentrant = 0;

static void __attribute__((constructor)) constructor() {
  if (initialized) {
    return;
  }
  if (sizeof((void*)0) != sizeof((size_t)0)) {
    fprintf(stderr, "BUG: expected size of size_t and void* to be the same.\n");
    exit(1);
  }
  void* lib = dlopen("target/debug/libpymemprofile_api.so", RTLD_LAZY | RTLD_DEEPBIND);
  if (!lib) {
    fprintf(stderr, "Couldn't load libpymemprofile_api.so library: %s\n", dlerror());
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

// TODO switch to new add_allocation and free_allocation APIs

// Override memory-allocation functions:
__attribute__ ((visibility("default"))) void* malloc(size_t size)  {
  void* result = __libc_malloc(size);
  if (size > 0 && result != NULL && !will_i_be_reentrant && initialized) {
    will_i_be_reentrant = 1;
    add_allocation_hook((size_t)result, size);
    will_i_be_reentrant = 0;
  }
  return result;
}

__attribute__ ((visibility("default"))) void* calloc(size_t nmemb, size_t size) {
  void* result = __libc_calloc(nmemb, size);
  size_t allocated = nmemb * size;
  if (allocated > 0 && !will_i_be_reentrant && initialized) {
    will_i_be_reentrant = 1;
    add_allocation_hook((size_t)result, allocated);
    will_i_be_reentrant = 0;
  }
  return result;
}

__attribute__ ((visibility("default"))) void free(void* addr) {
  if (!initialized) {
    // Well, we're going to leak a little memory, but, such is life...
    return;
  }
  underlying_real_free(addr);
  if (!will_i_be_reentrant && initialized) {
    will_i_be_reentrant = 1;
    free_allocation_hook((size_t)addr);
    will_i_be_reentrant = 0;
  }
}

/*
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
*/
