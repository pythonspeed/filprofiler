use ahash::RandomState as ARandomState;
use lazy_static::lazy_static;
use libc::{c_char, c_void, pid_t, pthread_t};
use parking_lot::{Mutex, RawMutex};
use pymemprofile_api::{
    memorytracking::{Callstack, FunctionId},
    performancetracking::{gettid, GlobalThreadId, PerfImpl, PerformanceTracker},
    python::get_callstack,
    util::new_hashmap,
};
use pyo3::{
    ffi::{PyCodeObject, PyFrameObject, PyObject},
    prelude::pyclass,
    AsPyPointer, Py, PyAny, PyResult, Python,
};
use std::{
    borrow::Borrow,
    collections::{BTreeMap, HashMap},
    ffi::CStr,
    path::PathBuf,
    sync::{
        atomic::{AtomicPtr, Ordering},
        Arc,
    },
};

use crate::{disable_memory_tracking, fil_decrement_reentrancy, fil_increment_reentrancy};

lazy_static! {
    // Map pthread_t to thread IDs (==process IDs in Linux), for use by
    // performance tracking.
    static ref PTHREAD_T_TO_TID: Mutex<HashMap<pthread_t, pid_t, ARandomState>> =
        Mutex::new(new_hashmap());
}

#[pyclass]
struct PerformanceTrackerWrapper {
    wrapped: Box<PerformanceTracker<FilPerfImpl>>,
}

#[no_mangle]
extern "C" fn fil_start_performance_tracking() -> *mut PyObject {
    let tracker = Box::new(PerformanceTracker::new(FilPerfImpl::new()));
    let wrapper = Python::with_gil(|py| -> PyResult<Py<PerformanceTrackerWrapper>> {
        let wrapper = Py::new(py, PerformanceTrackerWrapper { wrapped: tracker })?;
        Ok(wrapper)
    })
    .unwrap();
    let result = wrapper.as_ptr();
    std::mem::forget(wrapper);
    result
}

fn run_with_perf_tracker<F>(tracker: *mut PyObject, f: F)
where
    F: FnOnce(Python, &PerformanceTracker<FilPerfImpl>),
{
    // All these functions are called from CPython.
    unsafe {
        Python::with_gil_unchecked(|py| {
            let pt_wrapper: Py<PerformanceTrackerWrapper> =
                unsafe { Py::from_borrowed_ptr(py, tracker) };
            let tracker = &pt_wrapper.borrow(py).wrapped;
            f(py, tracker);
        });
    }
}

#[no_mangle]
extern "C" fn fil_stop_performance_tracking(tracker: *mut PyObject) {
    run_with_perf_tracker(tracker, |py, tracker| tracker.stop());
}

#[no_mangle]
extern "C" fn fil_dump_performance_tracking(tracker: *mut PyObject, path: *const c_char) {
    let path = PathBuf::from(
        unsafe { CStr::from_ptr(path) }
            .to_str()
            .expect("Path wasn't UTF-8"),
    );
    run_with_perf_tracker(tracker, |py, tracker| {
        py.allow_threads(|| {
            let memory_tracker = crate::TRACKER_STATE.lock();
            tracker.dump_profile(&path, &memory_tracker.allocations.functions);
        })
    });
}

#[no_mangle]
extern "C" fn pymemprofile_performance_push_frame(
    tracker: *mut PyObject,
    frame: *mut PyFrameObject,
) {
    run_with_perf_tracker(tracker, |_, tracker| {
        tracker.run_with_perf_impl(|perf_impl| perf_impl.push_frame(frame))
    });
}

#[no_mangle]
extern "C" fn pymemprofile_performance_pop_frame(
    tracker: *mut PyObject,
    parent_frame: *mut PyFrameObject,
) {
    run_with_perf_tracker(tracker, |_, tracker| {
        tracker.run_with_perf_impl(|perf_impl| perf_impl.pop_frame(parent_frame))
    });
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

lazy_static! {
    static ref ALL_THREAD_FRAMES: Mutex<BTreeMap<GlobalThreadId, Arc<AtomicPtr<PyFrameObject>>>> =
        Mutex::new(BTreeMap::new());
}

/// Stored as thread-local, tracked in THREAD_FRAMES.
struct PerThreadFrame {
    current_frame: Arc<AtomicPtr<PyFrameObject>>,
}

impl PerThreadFrame {
    fn new() -> Self {
        let result = Self {
            current_frame: Arc::default(),
        };
        ALL_THREAD_FRAMES
            .lock()
            .insert(gettid(), result.current_frame.clone());
        result
    }

    fn set(&self, frame: *mut PyFrameObject) {
        self.current_frame.store(frame, Ordering::Relaxed);
    }

    fn get(&self) -> *mut PyFrameObject {
        self.current_frame.load(Ordering::Relaxed)
    }
}

thread_local! { static THREAD_FRAME: PerThreadFrame = PerThreadFrame::new(); }

impl Drop for PerThreadFrame {
    fn drop(&mut self) {
        ALL_THREAD_FRAMES.lock().remove(&gettid());
    }
}

/// Implement PerfImpl for the open source Fil profiler.
struct FilPerfImpl {
    mutex: Mutex<()>,
}

// The main risk here is accessing PyFrameObject from non-GIL thread. We solve
// this by haveing our own lock around any access to it, including notably the
// GIL-thread's update in the tracer. The lock, specifically, is in
// PerformanceTracker.
unsafe impl Send for FilPerfImpl {}

impl FilPerfImpl {
    fn new() -> Self {
        Self {
            mutex: Mutex::new(()),
        }
    }

    /// Add a new frame for the current thread (which is presumed to own the
    /// GIL).
    pub fn push_frame(&self, new_frame: *mut PyFrameObject) {
        // If this doesn't make it to the performance polling thread, the
        // callstack will be truncated and not have the lower frames. But it
        // won't *segfault*, the old (parent) frame is still valid.
        THREAD_FRAME.with(|frame_ptr| frame_ptr.set(new_frame));
    }

    /// Switch to parent frame for the current thread (which is presumed to own
    /// the GIL).
    pub fn pop_frame(&self, parent_frame: *mut PyFrameObject) {
        // The current frame is being popped, so it's no longer valid. That
        // means we need to ensure that the performance polling thread isn't
        // using it before we remove it, so we use a lock this time around.
        let _guard = self.mutex.lock();
        // We rely on locking in performance thread to ensure happens-before, so
        // atomic can still be relaxed.
        THREAD_FRAME.with(|frame_ptr| frame_ptr.set(parent_frame));
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
        let _guard = self.mutex.lock();
        ALL_THREAD_FRAMES
            .lock()
            .iter()
            .map(|(tid, frame)| {
                (
                    (*tid).clone(),
                    // we rely on lock to make this sufficiently up-to-date.
                    get_callstack(frame.load(Ordering::Relaxed), get_function_id, true),
                )
            })
            .collect()
    }
}
