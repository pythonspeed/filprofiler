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
use pyo3::ffi::{
    PyCodeObject, PyFrameObject, PyInterpreterState, PyInterpreterState_ThreadHead, PyThreadState,
    PyThreadState_Next,
};
use pyo3::Python;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
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

    pub fn finish(&self, destination_directory: PathBuf) {
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

    /// Dump flamegraphs to disk.
    fn dump_flamegraphs(
        &self,
        path: &Path,
        to_be_post_processed: bool,
        functions: &FunctionLocations,
    ) {
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
            path,
            "performance",
            "Performance",
            "samples",
            to_be_post_processed,
            |tbpp, dest| write_lines(tbpp, dest),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::filter_to_useful_callstacks;
    use im::HashMap;
    use itertools::Itertools;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn filtering_of_callstacks(
            // Allocated bytes. Will use index as the memory address.
            allocated_sizes in prop::collection::vec(0..1000 as usize, 5..15000),
        ) {
            let total_size : usize = allocated_sizes.iter().sum();
            let total_size_99 = (99 * total_size) / 100;
            let callstacks = (&allocated_sizes).iter().enumerate();
            let filtered : HashMap<usize,usize>  = filter_to_useful_callstacks(callstacks, total_size).collect();
            let filtered_size :usize = filtered.values().into_iter().sum();
            if filtered_size >= total_size_99  {
                if filtered.len() > 100 {
                    // Removing any item should take us to or below 99%
                    for value in filtered.values() {
                        prop_assert!(filtered_size - *value <= total_size_99)
                    }
                }
            } else {
                // Cut out before 99%, so must be too many items
                prop_assert_eq!(filtered.len(), 10000);
                prop_assert_eq!(filtered_size, allocated_sizes.clone().iter().sorted_by(
                    |a, b| Ord::cmp(b, a)).take(10000).sum::<usize>());
            }
        }

    }
}
