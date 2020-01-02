#define _GNU_SOURCE

#include <stdlib.h>
#include <stdio.h>
#include <dlfcn.h>

// The real, underlying library calls:
static void* (*real_malloc)(size_t size) = 0;
static void* (*real_calloc)(size_t nmemb, size_t size) = 0;

// The internal API we're notifying of allocations:
static void (*priv_update_memory_usage)(void) = 0;

// Note whether we've been initialized yet or not:
static int initialized = 0;

/*
  Load a symbol from the actual place that provides it.
*/
static void* loadsym(const char* symbol)
{
  void* result = dlsym(RTLD_NEXT, symbol);
  if (!result) {
    fprintf(stderr, "Error loading symbol: %s\n", dlerror());
    exit(1);
  }
  return result;
}

// Load the underlying API:

static void constructor() __attribute__((constructor));

void constructor(void) {
  real_malloc = loadsym("malloc");
  real_calloc = loadsym("calloc");
  void* lib = dlopen("target/debug/libpymemprofile_api.so", RTLD_LAZY | RTLD_DEEPBIND);
  if (!lib) {
    fprintf(stderr, "Couldn't load libpymemprofile_api.so library: %s\n", dlerror());
    exit(1);
  }
  priv_update_memory_usage = dlsym(lib, "pymemprofile_update_memory_usage");
  if (!priv_update_memory_usage) {
    fprintf(stderr, "Couldn't load pymemprofile API function: %s\n", dlerror());
    exit(1);
  }
  initialized = 1;
}

__attribute__((visibility("default"))) void shim_update_memory_usage() {
  //priv_update_memory_usage();
  fprintf(stdout, "Hello!\n");
}

// Override memory-allocation functions:
__attribute__((visibility("default"))) void *malloc(size_t size) {
  return real_malloc(size);
}

__attribute__((visibility("default"))) void *calloc(size_t nmemb, size_t size) {
  return real_calloc(nmemb, size);
}
