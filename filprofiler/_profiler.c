#include "Python.h"
#include "frameobject.h"

extern void pymemprofile_start_call(const char *name);
extern void pymemprofile_finish_call(void);

int fil_tracer(PyObject *obj, PyFrameObject *frame, int what, PyObject *arg) {
  if (what == PyTrace_CALL) {
    pymemprofile_start_call(PyUnicode_AsUTF8(frame->f_code->co_name));
    return 0;
  }
  if (what == PyTrace_RETURN) {
    pymemprofile_finish_call();
    return 0;
  }
  return 0;
}

static PyObject* fil_start_tracing(PyObject *self, PyObject *args) {
  if (!PyArg_ParseTuple(args, ""))
    return NULL;
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
