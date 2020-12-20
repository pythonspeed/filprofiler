#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif
#define PY_SSIZE_T_CLEAN
#include "Python.h"

#if PY_VERSION_HEX < 0x03080000
#define Py_BytesMain _Py_UnixMain
#endif

int main(int argc, char *argv[]) {
  return Py_BytesMain(argc, argv);
}
