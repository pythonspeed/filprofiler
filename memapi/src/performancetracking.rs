// Performance profiling.
//
// TODO special handling for thread that has GIL when sampling happens
// TODO callstacks
// TODO thread status (CPU/Disk/Waiting/etc.)
// TODO dump on shutdown
// TODO non-Python threads

use crate::memorytracking::{Callstack, FunctionId};
use crate::python::get_callstack;

use super::util::new_hashmap;
use ahash::RandomState as ARandomState;
use pyo3::ffi::{
    PyCodeObject, PyFrameObject, PyInterpreterState, PyInterpreterState_ThreadHead, PyThreadState,
    PyThreadState_Next,
};
use pyo3::Python;
use std::collections::HashMap;
use std::ptr::null_mut;
use std::sync::atomic::{AtomicBool, Ordering};

/// Track what threads are doing over time.
pub struct PerformanceTracker {
    callstack_to_samples: HashMap<Callstack, usize, ARandomState>,
    running: AtomicBool,
}

// Requires Python 3.9 or later...
extern "C" {
    fn PyInterpreterState_Get() -> *mut PyInterpreterState;
    fn PyThreadState_GetFrame(ts: *mut PyThreadState) -> *mut PyFrameObject;
}

impl PerformanceTracker {
    pub fn new() -> Self {
        Self {
            callstack_to_samples: new_hashmap(),
            running: AtomicBool::new(true),
        }
    }

    pub fn finish(&self) {
        self.running.store(false, Ordering::Release);
    }

    fn run_sampling_thread<F>(&mut self, get_function_id: F)
    where
        F: Fn(*mut PyCodeObject) -> Option<FunctionId>,
    {
        let get_function_id = &get_function_id;
        while self.running.load(Ordering::Acquire) {
            std::thread::sleep(std::time::Duration::from_millis(50));
            Python::with_gil(|_py| {
                let interp = unsafe { PyInterpreterState_Get() };
                let mut tstate = unsafe { PyInterpreterState_ThreadHead(interp) };
                while tstate != null_mut() {
                    let frame = unsafe { PyThreadState_GetFrame(tstate) };
                    let callstack = get_callstack(frame, get_function_id);
                    *self.callstack_to_samples.entry(callstack).or_insert(0) += 1;
                    tstate = unsafe { PyThreadState_Next(tstate) };
                }
            });
        }
        // We're done, so dump profiling information to disk.
    }
}
