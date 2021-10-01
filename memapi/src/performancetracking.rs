// Performance profiling.
//
// TODO special handling for thread that has GIL when sampling happens
// TODO callstacks
// TODO thread status (CPU/Disk/Waiting/etc.)
// TODO dump on shutdown
// TODO non-Python threads

use crate::flamegraph::{filter_to_useful_callstacks, write_flamegraphs, write_lines};
use crate::memorytracking::{Callstack, FunctionId, FunctionLocations};
use crate::python::get_callstack;

use super::util::new_hashmap;
use ahash::RandomState as ARandomState;
use parking_lot::Mutex;
use pyo3::ffi::{
    PyCodeObject, PyFrameObject, PyInterpreterState, PyInterpreterState_ThreadHead, PyThreadState,
    PyThreadState_Next,
};
use pyo3::Python;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::ptr::null_mut;
use std::sync::Arc;
use std::thread::{spawn, JoinHandle};

// Requires Python 3.9 or later...
extern "C" {
    fn PyInterpreterState_Get() -> *mut PyInterpreterState;
    fn PyThreadState_GetFrame(ts: *mut PyThreadState) -> *mut PyFrameObject;
}

/// Track what threads are doing over time.
struct PerformanceTrackerInner {
    callstack_to_samples: HashMap<Callstack, usize, ARandomState>,
    running: bool,
}

struct PerformanceTracker {
    inner: Arc<Mutex<PerformanceTrackerInner>>,
}

impl PerformanceTracker {
    pub fn new<F>(get_function_id: F) -> Self
    where
        F: Send + Sync + 'static + Fn(*mut PyCodeObject) -> Option<FunctionId>,
    {
        let inner = Arc::new(Mutex::new(PerformanceTrackerInner::new()));
        let inner2 = inner.clone();
        spawn(move || {
            let get_function_id = &get_function_id;
            loop {
                std::thread::sleep(std::time::Duration::from_millis(50));
                // TODO make sure we don't get GIL/inner-lock deadlocks
                let mut inner = inner.lock();
                if !inner.is_running() {
                    break;
                }
                inner.add_samples(get_function_id);
            }
        });
        Self { inner: inner2 }
    }

    pub fn finish(&self, destination_directory: &Path, functions: &FunctionLocations) {
        let mut inner = self.inner.lock();
        inner.finish();
        inner.dump_flamegraphs(destination_directory, functions);
    }
}

impl PerformanceTrackerInner {
    fn new() -> Self {
        Self {
            callstack_to_samples: new_hashmap(),
            running: true,
        }
    }

    fn is_running(&self) -> bool {
        self.running
    }

    /// Finish running.
    fn finish(&mut self) {
        self.running = false;
    }

    /// Add samples for all threads.
    fn add_samples<F>(&mut self, get_function_id: F)
    where
        F: Fn(*mut PyCodeObject) -> Option<FunctionId>,
    {
        let get_function_id = &get_function_id;
        Python::with_gil(|_py| {
            let interp = unsafe { PyInterpreterState_Get() };
            let mut tstate = unsafe { PyInterpreterState_ThreadHead(interp) };
            while tstate != null_mut() {
                let frame = unsafe { PyThreadState_GetFrame(tstate) };
                let callstack = get_callstack(frame, get_function_id);
                self.add_sample(callstack);
                tstate = unsafe { PyThreadState_Next(tstate) };
            }
        });
    }

    /// Add a sample.
    fn add_sample(&mut self, callstack: Callstack) {
        let samples = self.callstack_to_samples.entry(callstack).or_insert(0);
        *samples += 1;
    }

    /// Dump flamegraphs to disk.
    fn dump_flamegraphs(&self, destination_directory: &Path, functions: &FunctionLocations) {
        let write_lines = |to_be_post_processed: bool, dest: &Path| {
            let total_samples = self.callstack_to_samples.values().sum();
            let lines =
                filter_to_useful_callstacks(self.callstack_to_samples.iter(), total_samples).map(
                    move |(callstack, calls)| {
                        format!(
                            "{} {}",
                            callstack.as_string(to_be_post_processed, &functions, ";"),
                            calls
                        )
                    },
                );
            write_lines(lines, dest)
        };

        write_flamegraphs(
            destination_directory,
            "performance",
            "Performance",
            "samples",
            true,
            |tbpp, dest| write_lines(tbpp, dest),
        )
    }
}
