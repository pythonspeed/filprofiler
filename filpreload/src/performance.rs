use ahash::RandomState as ARandomState;
use lazy_static::lazy_static;
use libc::{c_char, c_void, pid_t, pthread_t};
use parking_lot::Mutex;
use pymemprofile_api::{
    memorytracking::{Callstack, FunctionId},
    performancetracking::{gettid, GlobalThreadId, PerfImpl, PerformanceTracker},
    util::new_hashmap,
};
use pyo3::ffi::PyCodeObject;
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
extern "C" fn fil_stop_and_dump_performance_tracking(
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

/// Performance profiling.
extern "C" {
    fn get_pycodeobject_function_id(code: *mut PyCodeObject) -> u64;
}

// TODO This duplicates some code in the C codepath... should try and merge it.
fn get_function_id(code_object: *mut PyCodeObject) -> Option<FunctionId> {
    let mut function_id = unsafe { get_pycodeobject_function_id(code_object) };
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
    unsafe {
        fil_increment_reentrancy();
    }
    let pthread_id = unsafe { libc::pthread_self() };
    let pid: pid_t = gettid();
    let mut map = PTHREAD_T_TO_TID.lock();
    map.insert(pthread_id, pid);
    unsafe {
        fil_decrement_reentrancy();
    }
}

/// Implement PerfImpl for the open source Fil profiler.
struct FilPerfImpl {
    per_thread_frames: HashMap<GlobalThreadId, *mut PyFrameObject, ARandomState>,
}

impl PerfImpl for FilPerfImpl {
    type Iter = std::collections::hash_map::Iter<'static, GlobalThreadId, Callstack>;

    fn new() -> Self {
        Self {
            per_thread_frames: new_hashmap(),
        }
    }

    fn setup_running_thread(&self) {
        disable_memory_tracking();
    }

    fn get_callstacks(&self) -> Self::Iter {
        self.per_thread_frames.iter().map
    }
}
