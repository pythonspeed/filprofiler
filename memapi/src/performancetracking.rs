/*
Performance profiling.

DONE actually write callstacks
DONE fix segfault on shutdown
DONE disable memory tracking in polling thread
DONE write to correct directory
DONE add to HTML template
DONE acquiring GIL after 50ms on startup works... but it's fragile, should be _sure_ GIL is initialized?
DONE filter out the tracking thread from output
DONE unknown frames
DONE how to start/stop when using Fil's Python API? no global PERFORMANCE_TRACKER, instead create new PerformanceTracker when starting tracking, return it to Python! then stop it when we stop tracking.
DONE thread status (CPU/Disk/Waiting/etc.)
DONE dump on shutdown
TODO non-Python threads
DONE better title for SVG
TODO tests
TODO macos
*/

use crate::flamegraph::{filter_to_useful_callstacks, write_flamegraphs, write_lines};
use crate::memorytracking::{Callstack, FunctionLocations};

use super::util::new_hashmap;
use ahash::RandomState as ARandomState;
use libc::pid_t;
use parking_lot::Mutex;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::{Builder as ThreadBuilder, JoinHandle};
use sysinfo::{ProcessExt, ProcessStatus, System, SystemExt};

thread_local!(static TID: GlobalThreadId = GlobalThreadId((unsafe { libc::syscall(libc::SYS_gettid) }) as pid_t));

/// Get the current thread's id (==pid_t on Linux)
pub fn gettid() -> GlobalThreadId {
    // TODO macOS.
    TID.with(|tid| tid.clone())
}

/// OS-specific system-wide thread identifier, for use with platform per-thread
/// status APIs. TODO macOS
#[derive(Eq, PartialEq, Hash, Clone, Copy, PartialOrd, Ord)]
pub struct GlobalThreadId(pid_t);

/// Implementation-specific details.
pub trait PerfImpl {
    /// Typically this will disable things like memory tracking.
    fn setup_running_thread(&self);

    /// Get the callstacks for all threads.
    /// TODO at some point may want something more generic than Callstack.
    fn get_callstacks(&self) -> Vec<(GlobalThreadId, Callstack)>;
}

/// Track what threads are doing over time.
struct PerformanceTrackerInner<P: PerfImpl + Sync + Send> {
    callstack_to_samples: Mutex<HashMap<(Callstack, ThreadStatus), usize, ARandomState>>,
    should_stop: AtomicBool,
    perf_impl: P,
}

pub struct PerformanceTracker<P: PerfImpl + Sync + Send> {
    inner: Arc<PerformanceTrackerInner<P>>,
    handle: Mutex<Option<JoinHandle<()>>>,
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
            ThreadStatus::Uninterruptible => "\u{29D7} Uninterruptible wait",
            _ => "? Other",
        })
    }
}

impl<P: PerfImpl + Sync + Send + 'static> PerformanceTracker<P> {
    pub fn new(perf_impl: P) -> Self {
        let inner = Arc::new(PerformanceTrackerInner::new(perf_impl));
        let inner2 = inner.clone();
        let handle = ThreadBuilder::new()
            .name("PerformanceTracker".to_string())
            .spawn(move || {
                inner.perf_impl.setup_running_thread();
                loop {
                    std::thread::sleep(std::time::Duration::from_millis(47));

                    // TODO Kinda wierd factoring? Also this may be expensive,
                    // see if psutil is better or maybe just extrac the code we
                    // actually need, for status.
                    let pid = std::process::id() as i32;
                    let mut system = System::new();
                    system.refresh_process(pid);

                    if !inner.is_running() {
                        break;
                    }
                    inner.add_samples(system);
                }
            })
            .unwrap();
        Self {
            inner: inner2,
            handle: Mutex::new(Some(handle)),
        }
    }

    pub fn run_with_perf_impl<F>(&self, run: F)
    where
        F: FnOnce(&P),
    {
        let perf_impl = &self.inner.perf_impl;
        run(perf_impl)
    }

    pub fn stop(&self) {
        self.inner.finish();
        let handle = { self.handle.lock().take() };
        if let Some(handle) = handle {
            // TODO maybe log?
            let _ = handle.join();
        }
    }

    pub fn dump_profile(&self, destination_directory: &Path, functions: &FunctionLocations) {
        self.inner
            .dump_flamegraphs(destination_directory, functions);
    }
}

impl<P: PerfImpl + Sync + Send> PerformanceTrackerInner<P> {
    fn new(perf_impl: P) -> Self {
        Self {
            callstack_to_samples: Mutex::new(new_hashmap()),
            should_stop: AtomicBool::new(false),
            perf_impl,
        }
    }

    fn is_running(&self) -> bool {
        !self.should_stop.load(Ordering::Acquire)
    }

    /// Finish running.
    fn finish(&self) {
        self.should_stop.store(true, Ordering::Release)
    }

    /// Add samples for all threads.
    fn add_samples(&self, system: System) -> Option<()> {
        let pid = std::process::id() as i32;
        let process = system.process(pid)?;
        let this_thread_tid = gettid();

        let mut handled = HashSet::new();
        for (tid, callstack) in self.perf_impl.get_callstacks() {
            if tid != this_thread_tid {
                let thread = if process.pid() == tid.0 {
                    // The main thread
                    Some(process)
                } else {
                    // A child thread
                    process.tasks.get(&tid.0)
                };
                handled.insert(tid.0);
                let status = thread.map_or(ThreadStatus::Other, |p| p.status().into());
                self.add_sample(callstack, status);
            }
        }

        for tid in process.tasks.keys() {
            if !handled.contains(&tid) {
                // Not a Python thread, but we should still profile it.
                let status = process
                    .tasks
                    .get(tid)
                    .map_or(ThreadStatus::Other, |p| p.status().into());
                self.add_sample(Callstack::new(), status);
            }
        }

        Some(())
    }

    /// Add a sample.
    /// TODO move lock out
    fn add_sample(&self, callstack: Callstack, status: ThreadStatus) {
        let mut c_t_s = self.callstack_to_samples.lock();
        let samples = c_t_s.entry((callstack, status)).or_insert(0);
        *samples += 1;
    }

    /// Dump flamegraphs to disk.
    fn dump_flamegraphs(&self, destination_directory: &Path, functions: &FunctionLocations) {
        let c_t_s = self.callstack_to_samples.lock();
        let write_lines = |to_be_post_processed: bool, dest: &Path| {
            let total_samples = c_t_s.values().sum();
            let lines = filter_to_useful_callstacks(c_t_s.iter(), total_samples).map(
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
            "Performance: Combined per-thread runtime",
            "samples",
            true,
            |tbpp, dest| write_lines(tbpp, dest),
        )
    }
}
