use ahash::RandomState as ARandomState;
use lazy_static::lazy_static;
use libc::{c_char, c_void, pid_t, pthread_t};
use parking_lot::Mutex;
use pymemprofile_api::{
    memorytracking::{Callstack, FunctionId},
    performancetracking::{gettid, GlobalThreadId, PerfImpl, PerformanceTracker},
    python::get_callstack,
    util::new_hashmap,
};
use pyo3::ffi::{PyCodeObject, PyFrameObject};
use std::{collections::HashMap, ffi::CStr, path::PathBuf};

use crate::{disable_memory_tracking, fil_decrement_reentrancy, fil_increment_reentrancy};

lazy_static! {
    // Map pthread_t to thread IDs (==process IDs in Linux), for use by
    // performance tracking.
    static ref PTHREAD_T_TO_TID: Mutex<HashMap<pthread_t, pid_t, ARandomState>> =
        Mutex::new(new_hashmap());
}

#[no_mangle]
extern "C" fn fil_start_performance_tracking() -> *mut PerformanceTracker<FilPerfImpl> {
    let tracker = Box::new(PerformanceTracker::new(FilPerfImpl::new()));
    Box::into_raw(tracker)
}

#[no_mangle]
extern "C" fn fil_stop_performance_tracking(tracker: *mut PerformanceTracker<FilPerfImpl>) {
    unsafe {
        fil_increment_reentrancy();
    }
    let performance_tracker = unsafe { tracker.as_ref().unwrap() };
    performance_tracker.stop();
    unsafe {
        fil_decrement_reentrancy();
    }
}

#[no_mangle]
extern "C" fn fil_dump_performance_tracking(
    tracker: *mut PerformanceTracker<FilPerfImpl>,
    path: *const c_char,
) {
    unsafe {
        fil_increment_reentrancy();
    }
    let performance_tracker = unsafe { Box::from_raw(tracker) };
    let path = PathBuf::from(
        unsafe { CStr::from_ptr(path) }
            .to_str()
            .expect("Path wasn't UTF-8"),
    );

    pyo3::Python::with_gil(|py| {
        py.allow_threads(|| {
            let memory_tracker = crate::TRACKER_STATE.lock();
            performance_tracker.dump_profile(&path, &memory_tracker.allocations.functions);
        })
    });
    unsafe {
        fil_decrement_reentrancy();
    }
}

#[no_mangle]
extern "C" fn pymemprofile_performance_push_frame(
    tracker: *mut PerformanceTracker<FilPerfImpl>,
    frame: *mut PyFrameObject,
) {
    unsafe {
        fil_increment_reentrancy();
    }
    let performance_tracker = unsafe { tracker.as_ref().unwrap() };
    performance_tracker.run_with_perf_impl(|perf_impl| perf_impl.push_frame(frame));
    unsafe {
        fil_decrement_reentrancy();
    }
}

#[no_mangle]
extern "C" fn pymemprofile_performance_pop_frame(
    tracker: *mut PerformanceTracker<FilPerfImpl>,
    parent_frame: *mut PyFrameObject,
) {
    unsafe {
        fil_increment_reentrancy();
    }
    let performance_tracker = unsafe { tracker.as_ref().unwrap() };
    performance_tracker.run_with_perf_impl(|perf_impl| perf_impl.pop_frame(parent_frame));
    unsafe {
        fil_decrement_reentrancy();
    }
}

/// Performance profiling.
extern "C" {
    fn get_pycodeobject_function_id(code: *mut PyCodeObject) -> u64;
}

// TODO This duplicates some code in the C codepath... should try and merge it.
fn get_function_id(code_object: *mut PyCodeObject) -> Option<FunctionId> {
    let function_id = unsafe { get_pycodeobject_function_id(code_object) };
    if function_id == 0 {
        None
    } else {
        Some(FunctionId::new((function_id.saturating_sub(1)) as u32))
    }
}

fn pthread_t_to_tid(pthread_id: pthread_t) -> pid_t {
    let map = PTHREAD_T_TO_TID.lock();
    *map.get(&pthread_id).unwrap_or(&0)
}

// Keep track of mapping between pthread_t and pid_t.
#[no_mangle]
extern "C" fn pymemprofile_new_thread() {
    // TODO can rip this out
}

/// Implement PerfImpl for the open source Fil profiler.
struct FilPerfImpl {
    per_thread_frames: HashMap<GlobalThreadId, *mut PyFrameObject, ARandomState>,
}

// The main risk here is accessing PyFrameObject from non-GIL thread. We solve
// this by haveing our own lock around any access to it, including notably the
// GIL-thread's update in the tracer. The lock, specifically, is in
// PerformanceTracker.
unsafe impl Send for FilPerfImpl {}

impl FilPerfImpl {
    fn new() -> Self {
        Self {
            per_thread_frames: new_hashmap(),
        }
    }

    /// Add a new frame for the current thread (which is presumed to own the
    /// GIL).
    pub fn push_frame(&mut self, new_frame: *mut PyFrameObject) {
        self.per_thread_frames.insert(gettid(), new_frame);
    }

    /// Switch to parent frame for the current thread (which is presumed to own
    /// the GIL).
    pub fn pop_frame(&mut self, parent_frame: *mut PyFrameObject) {
        self.per_thread_frames.insert(gettid(), parent_frame);
    }
}

impl PerfImpl for FilPerfImpl {
    fn setup_running_thread(&self) {
        disable_memory_tracking();
    }

    /// This will be called with GIL _not_ owned, but lock prevents frame from
    /// changing out from under us, and the bits we care about (code object and
    /// line number) won't change. Or, rather, code object won't. Line number is
    /// iffy and might be wrong, but we're assuming get_callstack() can handle
    /// that.
    fn get_callstacks(&self) -> Vec<(GlobalThreadId, Callstack)> {
        self.per_thread_frames
            .iter()
            .map(|(tid, frame)| ((*tid).clone(), get_callstack(*frame, get_function_id, true)))
            .collect()
    }
}
