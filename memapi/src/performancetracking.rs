/*
Performance profiling.

DONE actually write callstacks
TODO fix segfault on shutdown
DONE disable memory tracking in polling thread
DONE write to correct directory
DONE add to HTML template
DONE acquiring GIL after 50ms on startup works... but it's fragile, should be _sure_ GIL is initialized?
DONE filter out the tracking thread from output
DONE unknown frames
TODO how to start/stop when using Fil's Python API? no global PERFORMANCE_TRACKER, instead create new PerformanceTracker when starting tracking, return it to Python! then stop it when we stop tracking.
TODO special handling for thread that has GIL when sampling happens
DONE thread status (CPU/Disk/Waiting/etc.)
TODO dump on shutdown
TODO non-Python threads
TODO better title for SVG
TODO Python < 3.9. Just disable?
TODO current mechanism loses thread-callstack-persistence Fil provides for non-Python threads. probably follow-up issue.
*/

use crate::flamegraph::{filter_to_useful_callstacks, write_flamegraphs, write_lines};
use crate::memorytracking::{Callstack, FunctionId, FunctionLocations};
use crate::python::get_callstack;

use super::util::new_hashmap;
use ahash::RandomState as ARandomState;
use libc::{pid_t, pthread_t};
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
use std::thread::{Builder as ThreadBuilder, JoinHandle};
use sysinfo::{ProcessExt, ProcessStatus, System, SystemExt};

// Requires Python 3.9 or later...
extern "C" {
    // From Python itself.
    fn PyInterpreterState_Get() -> *mut PyInterpreterState;
    fn PyThreadState_GetFrame(ts: *mut PyThreadState) -> *mut PyFrameObject;

    // APIs we provide.
    fn PyThreadState_GetPthreadId(ts: *mut PyThreadState) -> pthread_t;
}

/// Get the current thread's id (==pid_t on Linux)
pub fn gettid() -> pid_t {
    // TODO macOS.
    (unsafe { libc::syscall(libc::SYS_gettid) }) as pid_t
}

/// Track what threads are doing over time.
struct PerformanceTrackerInner {
    callstack_to_samples: HashMap<(Callstack, ThreadStatus), usize, ARandomState>,
    running: bool,
}

pub struct PerformanceTracker {
    inner: Arc<Mutex<(Option<JoinHandle<()>>, PerformanceTrackerInner)>>,
}

#[derive(Eq, PartialEq, Hash)]
enum ThreadStatus {
    Running,
    Waiting,
    Uninterruptible,
    Other,
}

impl From<ProcessStatus> for ThreadStatus {
    fn from(sp: ProcessStatus) -> Self {
        match sp {
            ProcessStatus::Run => ThreadStatus::Running,
            ProcessStatus::Idle => ThreadStatus::Uninterruptible,
            ProcessStatus::Sleep => ThreadStatus::Waiting,
            _ => ThreadStatus::Other,
        }
    }
}

impl std::fmt::Display for ThreadStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ThreadStatus::Running => "\u{2BC8} Running",
            ThreadStatus::Waiting => "\u{29D7} Waiting",
            ThreadStatus::Uninterruptible => "Uninterruptable wait",
            _ => "Other",
        })
    }
}

impl PerformanceTracker {
    pub fn new<S, GFI, PTP>(setup_thread: S, get_function_id: GFI, pthread_t_to_tid: PTP) -> Self
    where
        S: Send + Sync + 'static + FnOnce(),
        GFI: Send + Sync + 'static + Fn(*mut PyCodeObject) -> Option<FunctionId>,
        PTP: Send + Sync + 'static + Fn(pthread_t) -> pid_t,
    {
        // Make sure our pthread_t -> tid mapping works correctly.
        assert_eq!(unsafe { pthread_t_to_tid(libc::pthread_self()) }, gettid());

        let inner = Arc::new(Mutex::new((None, PerformanceTrackerInner::new())));
        let inner2 = inner.clone();
        let handle = ThreadBuilder::new()
            .name("PerformanceTracker".to_string())
            .spawn(move || {
                setup_thread();
                let get_function_id = &get_function_id;
                let pthread_t_to_tid = &pthread_t_to_tid;
                loop {
                    std::thread::sleep(std::time::Duration::from_millis(50));
                    // TODO make sure we don't get GIL/inner-lock deadlocks
                    let mut inner = inner.lock();
                    if !inner.1.is_running() {
                        break;
                    }
                    inner.1.add_samples(get_function_id, pthread_t_to_tid);
                }
            });
        {
            let mut inner = inner2.lock();
            inner.0 = Some(handle.unwrap());
        }
        Self { inner: inner2 }
    }

    pub fn dump_profile(self, destination_directory: &Path, functions: &FunctionLocations) {
        let handle = {
            let mut inner = self.inner.lock();
            inner.1.finish();
            inner.0.take()
        };
        if let Some(handle) = handle {
            handle.join();
        }
        let inner = self.inner.lock();
        inner.1.dump_flamegraphs(destination_directory, functions);
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
    fn add_samples<GFI, PTP>(&mut self, get_function_id: GFI, pthread_t_to_tid: PTP) -> Option<()>
    where
        GFI: Fn(*mut PyCodeObject) -> Option<FunctionId>,
        PTP: Fn(pthread_t) -> pid_t,
    {
        let pid = std::process::id() as i32;
        let mut system = System::new();
        system.refresh_process(pid);
        let process = system.process(pid)?;
        let this_thread_tid = gettid();
        let get_function_id = &get_function_id;
        Python::with_gil(|_py| {
            let interp = unsafe { PyInterpreterState_Get() };
            let mut tstate = unsafe { PyInterpreterState_ThreadHead(interp) };
            while tstate != null_mut() {
                let frame = unsafe { PyThreadState_GetFrame(tstate) };
                let callstack = get_callstack(frame, get_function_id, true);
                let tid = pthread_t_to_tid(unsafe { PyThreadState_GetPthreadId(tstate) });
                if tid != this_thread_tid {
                    let thread = if process.pid() == tid {
                        // The main thread
                        Some(process)
                    } else {
                        // A child thread
                        process.tasks.get(&tid)
                    };
                    let status = thread.map_or(ThreadStatus::Other, |p| p.status().into());
                    self.add_sample(callstack, status);
                }
                tstate = unsafe { PyThreadState_Next(tstate) };
            }
        });
        Some(())
    }

    /// Add a sample.
    fn add_sample(&mut self, callstack: Callstack, status: ThreadStatus) {
        let samples = self
            .callstack_to_samples
            .entry((callstack, status))
            .or_insert(0);
        *samples += 1;
    }

    /// Dump flamegraphs to disk.
    fn dump_flamegraphs(&self, destination_directory: &Path, functions: &FunctionLocations) {
        let write_lines = |to_be_post_processed: bool, dest: &Path| {
            let total_samples = self.callstack_to_samples.values().sum();
            let lines =
                filter_to_useful_callstacks(self.callstack_to_samples.iter(), total_samples).map(
                    move |((callstack, status), calls)| {
                        format!(
                            "{};{} {}",
                            callstack.as_string(to_be_post_processed, &functions, ";"),
                            status,
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
