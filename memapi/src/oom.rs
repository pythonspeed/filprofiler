use std::fs::read_to_string;

/// Logic for handling out-of-memory situations.

// Anything less than this is too dangerous, and we should dump and
// exit.
const MINIMAL_FREE: usize = 100 * 1024 * 1024;

pub trait MemoryInfo {
    /// Return how much memory we have, as bytes.
    fn get_available_memory(&self) -> usize;
    /// Return how much process memory is resident, as bytes.
    fn get_resident_process_memory(&self) -> usize;
}

/// Estimate whether we're about to run out of memory.
///
/// First, we need to define what "running out of memory" means. As a first
/// pass, 100MB or less of non-swap memory availability, minimum of OS in
/// general and current cgroup. Don't count swap because goal is
/// to avoid slowness, if someone wants to fallback to disk they should use
/// mmap().
///
/// This will break on over-committing, but... we can live with that.
///
/// macOS is very aggressive about swapping, so we add second heuristic: swap
/// for the process is bigger than available memory. This suggests large
/// pressure to swap, since the process wouldn't fit in memory on its own.
///
/// Second, we probably don't want to check every time, that's expensive. So
/// check every 1% of allocations remaining until we run out of available memory
/// (we don't even check for free()s, which just means more frequent checks).
pub struct OutOfMemoryEstimator<M: MemoryInfo> {
    // How many bytes it takes until we check again: whenever it's reset, it
    // starts as 1% of available memory.
    check_threshold_bytes: usize,
    // Pluggable way to get memory usage of the system and process.
    memory_info: M,
}

impl<M: MemoryInfo> OutOfMemoryEstimator<M> {
    pub fn new(memory_info: M) -> Self {
        Self {
            check_threshold_bytes: 0,
            memory_info,
        }
    }

    /// Check if we're (close to being) out of memory.
    pub fn are_we_oom(&mut self, total_allocated_bytes: usize) -> bool {
        let available_bytes = self.memory_info.get_available_memory();

        // Check if we're in danger zone, with very low available memory:
        if available_bytes < MINIMAL_FREE {
            return true;
        }

        // Check if we're excessively swapping. On macOS in particular there is
        // a strong tendency to go to swap (coupled with difficulty getting swap
        // numbers for a process). So if swap is bigger than available bytes,
        // we'll assume we're effectively OOM on theory that extensive swapping
        // is highly undesirable. We calculate relevant swap by subtracting
        // resident memory from the memory we know we've allocated.
        let rss = self.memory_info.get_resident_process_memory();
        // Because we don't track all allocations, technically resident memory
        // might be larger than what we think we allocated!
        if rss < total_allocated_bytes && (total_allocated_bytes - rss) > available_bytes {
            return true;
        }

        // Still have enough, so threshold to 1% to running out altogether. If
        // we're at 101MB free, this will check basically at the boundary.
        // Anything higher and we'll check even farther away, so it's still
        // safe, and this prevents us from checking too often when we're close,
        // as in an earlier iteration of this check.
        //
        // What if someone allocations 80MB when we're 120MB from running out?
        // See add_allocation() in memorytracking.rs, which will just immediatly
        // free that memory again since we're going to exit anyway.
        self.check_threshold_bytes = available_bytes / 100;

        // We're not OOM:
        false
    }

    /// Given new allocation size and total allocated bytes for the process,
    /// return whether we're out-of-memory. Only checks actual memory
    /// availability intermittently, as an optimization.
    pub fn too_big_allocation(
        &mut self,
        allocated_bytes: usize,
        total_allocated_bytes: usize,
    ) -> bool {
        let current_threshold = self.check_threshold_bytes;
        if allocated_bytes > current_threshold {
            // We've allocated enough that it's time to check for potential OOM
            // condition.
            return self.are_we_oom(total_allocated_bytes);
        }
        self.check_threshold_bytes = current_threshold - allocated_bytes;
        return false;
    }
}

#[cfg(target_os = "linux")]
fn get_cgroup_paths<'a>(proc_cgroups: &'a str) -> Vec<&'a str> {
    let mut result = vec![];
    for line in proc_cgroups.lines() {
        // TODO better error handling?
        let mut parts = line.splitn(3, ":");
        let subsystems = parts.nth(1).unwrap();
        if (subsystems == "") || subsystems.split(",").any(|s| s == "memory") {
            let cgroup_path = parts.nth(0).unwrap().strip_prefix("/").unwrap();
            result.push(cgroup_path);
        }
    }
    result
}

/// Real system information.
pub struct RealMemoryInfo {
    // The current process.
    process: psutil::process::Process,
    // On Linux, the current cgroup _at startup_. If it changes after startup,
    // we'll be wrong, but that's unlikely.
    #[cfg(target_os = "linux")]
    cgroup: Option<cgroups_rs::Cgroup>,
}

impl RealMemoryInfo {
    #[cfg(target_os = "linux")]
    pub fn new() -> Self {
        let get_cgroup = || {
            let contents = match read_to_string("/proc/self/cgroup") {
                Ok(contents) => contents,
                Err(err) => {
                    eprintln!("=fil-profile= Couldn't read /proc/self/cgroup ({:})", err);
                    return None;
                }
            };
            let cgroup_paths = get_cgroup_paths(&contents);
            for path in cgroup_paths {
                let h = cgroups_rs::hierarchies::auto();
                return Some(cgroups_rs::Cgroup::load(h, path));
            }
            None
        };
        return Self {
            cgroup: get_cgroup(),
            process: psutil::process::Process::current().unwrap(),
        };
    }

    #[cfg(target_os = "macos")]
    pub fn new() -> Self {
        Self {
            process: psutil::process::Process::current().unwrap(),
        }
    }

    #[cfg(target_os = "linux")]
    pub fn get_cgroup_available_memory(&self) -> usize {
        let mut result = std::usize::MAX;
        if let Some(cgroup) = &self.cgroup {
            let mem: &cgroups_rs::memory::MemController = cgroup.controller_of().unwrap();
            let mem = mem.memory_stat();
            result = std::cmp::min(
                result,
                (mem.limit_in_bytes - mem.usage_in_bytes as i64) as usize,
            );
        }
        result
    }

    #[cfg(target_os = "macos")]
    pub fn get_cgroup_available_memory(&self) -> usize {
        std::usize::MAX
    }
}

impl MemoryInfo for RealMemoryInfo {
    /// Return how much free memory we have, as bytes.
    fn get_available_memory(&self) -> usize {
        // This will include memory that can become available by syncing
        // filesystem buffers to disk, which is probably what we want.
        let available = psutil::memory::virtual_memory().unwrap().available() as usize;
        let cgroup_available = self.get_cgroup_available_memory();
        std::cmp::min(available, cgroup_available)
    }

    fn get_resident_process_memory(&self) -> usize {
        self.process.memory_info().unwrap().rss() as usize
    }
}

#[cfg(test)]
mod tests {
    use super::{MemoryInfo, OutOfMemoryEstimator};
    use std::cell::Ref;
    use std::cell::RefCell;

    struct FakeMemory {
        available_memory: RefCell<usize>,
        swap: RefCell<usize>,
        checks: RefCell<Vec<usize>>,
    }

    impl FakeMemory {
        fn new() -> Self {
            FakeMemory {
                available_memory: RefCell::new(1_000_000_000),
                checks: RefCell::new(vec![]),
                swap: RefCell::new(0),
            }
        }

        fn allocate(&self, size: usize) {
            let mut mem = self.available_memory.borrow_mut();
            *mem -= size;
        }

        fn add_swap(&self, size: usize) {
            *self.swap.borrow_mut() += size;
        }

        fn get_checks(&self) -> Ref<Vec<usize>> {
            self.checks.borrow()
        }

        fn get_allocated(&self) -> usize {
            1_000_000_000 - *self.available_memory.borrow()
        }
    }

    impl MemoryInfo for FakeMemory {
        fn get_available_memory(&self) -> usize {
            self.checks
                .borrow_mut()
                .push(*self.available_memory.borrow());
            *self.available_memory.borrow()
        }

        fn get_resident_process_memory(&self) -> usize {
            self.get_allocated() - *self.swap.borrow()
        }
    }

    fn setup_estimator() -> OutOfMemoryEstimator<FakeMemory> {
        let fake_memory = FakeMemory::new();
        OutOfMemoryEstimator::new(fake_memory)
    }

    // We're out of memory if we're below the threshold.
    #[test]
    fn oom_threshold() {
        let mut estimator = setup_estimator();
        assert!(!estimator.are_we_oom(estimator.memory_info.get_allocated()));
        estimator.memory_info.allocate(500_000_000);
        assert!(!estimator.are_we_oom(estimator.memory_info.get_allocated()));
        estimator.memory_info.allocate(350_000_000);
        assert!(!estimator.are_we_oom(estimator.memory_info.get_allocated()));
        estimator.memory_info.allocate(50_000_000);
        // Now that we're below the maximum, we've gone too far:
        assert!(estimator.are_we_oom(estimator.memory_info.get_allocated()));
        estimator.memory_info.allocate(40_000_000);
        assert!(estimator.are_we_oom(estimator.memory_info.get_allocated()));
    }

    // We're out of memory if swap > available.
    #[test]
    fn oom_swap() {
        let mut estimator = setup_estimator();
        estimator.memory_info.allocate(500_000_001);
        assert!(!estimator.are_we_oom(estimator.memory_info.get_allocated()));

        estimator.memory_info.add_swap(499_999_999);
        assert!(!estimator.are_we_oom(estimator.memory_info.get_allocated()));

        estimator.memory_info.add_swap(2);
        assert!(estimator.are_we_oom(estimator.memory_info.get_allocated()));
    }

    // The intervals between checking if out-of-memory shrink as we get closer
    // to running out of memory
    #[test]
    fn oom_estimator_shrinking_intervals() {
        let mut estimator = setup_estimator();
        loop {
            {
                let memory = &mut estimator.memory_info;
                memory.allocate(10_000);
            }
            if estimator.too_big_allocation(10_000, estimator.memory_info.get_allocated()) {
                break;
            }
            // by 100MB we should have detected OOM.
            assert!(*(&estimator.memory_info).available_memory.borrow() >= 99_000_000);
        }
        let fake_memory = estimator.memory_info;
        let checks = fake_memory.get_checks();
        // Each check should come closer than the next:
        for pair in checks.windows(2) {
            assert!(pair[0] >= pair[1], "{} vs {}", pair[0], pair[1]);
        }
        // In the beginning we check infrequently:
        assert!((checks[0] - checks[1]) > 9_000_000);
        // By the end we should be checking more frequently:
        let final_difference = checks[checks.len() - 2] - checks[checks.len() - 1];
        assert!(
            final_difference < 1_100_000,
            "final difference: {}",
            final_difference,
        );
    }
}
