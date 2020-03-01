#include "Python.h"
#include "frameobject.h"
#include <stdint.h>
#include <stdio.h>

extern void fil_start_call(const char *file_name, const char *function_name,
                           uint32_t line_number);
extern void fil_finish_call(void);
extern void fil_new_line_number(uint16_t line_number);
extern void fil_thread_started();
extern void fil_thread_finished();

int fil_tracer(PyObject *obj, PyFrameObject *frame, int what, PyObject *arg) {
  switch (what) {
  case PyTrace_CALL:
    fil_start_call(PyUnicode_AsUTF8(frame->f_code->co_filename),
                   PyUnicode_AsUTF8(frame->f_code->co_name), frame->f_lineno);
    break;
  case PyTrace_RETURN:
    fil_finish_call();
    if (frame->f_back == NULL) {
      // This thread is done.
      fil_thread_finished();
    }
  default:
    break;
  }
  return 0;
}

static PyObject *fil_start_tracing(PyObject *self, PyObject *args) {
  if (!PyArg_ParseTuple(args, ""))
    return NULL;
  fil_thread_started();
  PyEval_SetProfile(fil_tracer, Py_None);
  return Py_None;
}

static PyMethodDef ProfilerMethods[] = {
    {"start_tracing", fil_start_tracing, METH_VARARGS, "Start tracing."},
    {NULL, NULL, 0, NULL}};

static struct PyModuleDef profilermodule = {PyModuleDef_HEAD_INIT,
                                            "_profiler", /* name of module */
                                            NULL, -1, ProfilerMethods};
PyMODINIT_FUNC PyInit__profiler(void) {
  return PyModule_Create(&profilermodule);
}
