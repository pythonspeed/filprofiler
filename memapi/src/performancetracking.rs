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
TODO better title for SVG
TODO Python < 3.9. Just disable?
TODO current mechanism loses thread-callstack-persistence Fil provides for non-Python threads. probably follow-up issue.
TODO tests
TODO macos
*/

use crate::flamegraph::{filter_to_useful_callstacks, write_flamegraphs, write_lines};
use crate::memorytracking::{Callstack, FunctionLocations};

use super::util::new_hashmap;
use ahash::RandomState as ARandomState;
use libc::pid_t;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;
use std::thread::{Builder as ThreadBuilder, JoinHandle};
use sysinfo::{ProcessExt, ProcessStatus, System, SystemExt};

/// Get the current thread's id (==pid_t on Linux)
pub fn gettid() -> GlobalThreadId {
    // TODO macOS.
    GlobalThreadId((unsafe { libc::syscall(libc::SYS_gettid) }) as pid_t)
}

/// OS-specific system-wide thread identifier, for use with platform per-thread
/// status APIs. TODO macOS
#[derive(Eq, PartialEq, Hash, Clone)]
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
struct PerformanceTrackerInner<P: PerfImpl> {
    callstack_to_samples: HashMap<(Callstack, ThreadStatus), usize, ARandomState>,
    running: bool,
    perf_impl: P,
}

pub struct PerformanceTracker<P: PerfImpl> {
    inner: Arc<Mutex<(Option<JoinHandle<()>>, PerformanceTrackerInner<P>)>>,
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

impl<P: PerfImpl + Send + 'static> PerformanceTracker<P> {
    pub fn new(perf_impl: P) -> Self {
        let inner = Arc::new(Mutex::new((None, PerformanceTrackerInner::new(perf_impl))));
        let inner2 = inner.clone();
        let handle = ThreadBuilder::new()
            .name("PerformanceTracker".to_string())
            .spawn(move || {
                inner.lock().1.perf_impl.setup_running_thread();
                loop {
                    std::thread::sleep(std::time::Duration::from_millis(47));
                    let mut inner = inner.lock();
                    if !inner.1.is_running() {
                        break;
                    }
                    inner.1.add_samples();
                }
            });
        {
            let mut inner = inner2.lock();
            inner.0 = Some(handle.unwrap());
        }
        Self { inner: inner2 }
    }

    pub fn run_with_perf_impl<F>(&self, run: F)
    where
        F: FnOnce(&mut P),
    {
        let mut inner = self.inner.lock();
        let perf_impl = &mut inner.1.perf_impl;
        run(perf_impl)
    }

    pub fn stop(&self) {
        let handle = {
            let mut inner = self.inner.lock();
            inner.1.finish();
            inner.0.take()
        };
        if let Some(handle) = handle {
            // TODO maybe log?
            let _ = handle.join();
        }
    }

    pub fn dump_profile(&self, destination_directory: &Path, functions: &FunctionLocations) {
        let inner = self.inner.lock();
        inner.1.dump_flamegraphs(destination_directory, functions);
    }
}

impl<P: PerfImpl> PerformanceTrackerInner<P> {
    fn new(perf_impl: P) -> Self {
        Self {
            callstack_to_samples: new_hashmap(),
            running: true,
            perf_impl,
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
    fn add_samples(&mut self) -> Option<()> {
        let pid = std::process::id() as i32;
        let mut system = System::new();
        system.refresh_process(pid);
        let process = system.process(pid)?;
        let this_thread_tid = gettid();

        for (tid, callstack) in self.perf_impl.get_callstacks() {
            if tid != this_thread_tid {
                let thread = if process.pid() == tid.0 {
                    // The main thread
                    Some(process)
                } else {
                    // A child thread
                    process.tasks.get(&tid.0)
                };
                let status = thread.map_or(ThreadStatus::Other, |p| p.status().into());
                self.add_sample(callstack, status);
            }
        }

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
