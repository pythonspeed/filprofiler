use once_cell::sync::Lazy;
use parking_lot::Mutex;
use std::fs::read_to_string;

/// Logic for handling out-of-memory situations.

// Anything less than this is too dangerous, and we should dump and
// exit.
const MINIMAL_FREE: usize = 100 * 1024 * 1024;

/// Estimate whether we're about to run out of memory.
///
/// First, we need to define what "running out of memory" means. As a first
/// pass, 100MB or less of non-swap memory availability, minimum of OS in
/// general, current cgroup, and rusage limit. Don't count swap because goal is
/// to avoid slowness, if someone wants to fallback to disk they should use
/// mmap().
///
/// This will break on over-committing, but... we can live with that.
///
/// Second, we probably don't want to check every time, that's expensive. So
/// check every 1% of allocations remaining until we hit the danger zone (we
/// don't even check for free()s, which just means more frequent checks).
pub struct OutOfMemoryEstimator {
    // How many bytes it takes until we check again: initially, 1% of distance
    // between danger zone and free memory as of last check.
    check_threshold_bytes: usize,
    // Callable that returns currently free bytes in RAM (w/o swap). In practice
    // this may take into account things like rusage and cgroups limits, which
    // may be lower than actual free RAM.
    get_available_bytes: Box<dyn Fn() -> usize + Send + 'static>,
}

impl OutOfMemoryEstimator {
    pub fn new<F: Fn() -> usize + Send + 'static>(get_available_bytes: F) -> Self {
        Self {
            check_threshold_bytes: 0,
            get_available_bytes: Box::new(get_available_bytes),
        }
    }

    /// Check if we're (close to being) out of memory.
    pub fn are_we_oom(&mut self) -> bool {
        // Figure out how much is free, reset the threshold accordingly.
        let available_bytes = (self.get_available_bytes)();

        // Check if we're out of memory:
        if available_bytes < MINIMAL_FREE {
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

    /// Given new allocation size, return whether we're out-of-memory. May or
    /// may not actually check current free memory, as an optimization.
    pub fn too_big_allocation(&mut self, allocated_bytes: usize) -> bool {
        let current_threshold = self.check_threshold_bytes;
        if allocated_bytes > current_threshold {
            // We've allocated enough that it's time to check for potential OOM
            // condition.
            return self.are_we_oom();
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

#[cfg(target_os = "linux")]
pub fn get_cgroup_available_memory() -> usize {
    static MAYBE_CGROUP: Lazy<Option<Mutex<cgroups_rs::Cgroup>>> = Lazy::new(|| {
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
            return Some(Mutex::new(cgroups_rs::Cgroup::load(h, path)));
        }
        None
    });
    let mut result = std::usize::MAX;
    if let Some(cgroup) = &*MAYBE_CGROUP {
        let cgroup = cgroup.lock();
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
pub fn get_cgroup_available_memory() -> usize {
    std::usize::MAX
}

/// Return how much free memory we have, as bytes.
pub fn get_available_memory() -> usize {
    // This will include memory that can become available by syncing
    // filesystem buffers to disk, which is probably what we want.
    let available = psutil::memory::virtual_memory().unwrap().available() as usize;
    let cgroup_available = get_cgroup_available_memory();
    std::cmp::min(available, cgroup_available)
}

#[cfg(test)]
mod tests {
    use super::OutOfMemoryEstimator;
    use std::sync::{Arc, Mutex};

    struct FakeMemory {
        available_memory: usize,
        checks: Vec<usize>,
    }

    impl FakeMemory {
        fn new() -> Self {
            FakeMemory {
                available_memory: 1_000_000_000,
                checks: vec![],
            }
        }

        fn get_available_memory(&mut self) -> usize {
            self.checks.push(self.available_memory);
            self.available_memory
        }

        fn allocate(&mut self, size: usize) {
            self.available_memory -= size;
        }

        fn get_checks(&self) -> &[usize] {
            &self.checks
        }
    }

    fn setup_estimator() -> (Arc<Mutex<FakeMemory>>, OutOfMemoryEstimator) {
        let fake_memory = Arc::new(Mutex::new(FakeMemory::new()));
        let get_memory = {
            let fake_memory = fake_memory.clone();
            move || -> usize {
                let mut memory = fake_memory.lock().unwrap();
                memory.get_available_memory()
            }
        };
        let estimator = OutOfMemoryEstimator::new(get_memory);
        (fake_memory, estimator)
    }

    // We're out of memory if we're below the threshold.
    #[test]
    fn oom_threshold() {
        let (fake_memory, mut estimator) = setup_estimator();
        let allocate = |amount| {
            let mut memory = fake_memory.lock().unwrap();
            memory.allocate(amount);
        };
        assert!(!estimator.are_we_oom());
        allocate(500_000_000);
        assert!(!estimator.are_we_oom());
        allocate(350_000_000);
        assert!(!estimator.are_we_oom());
        allocate(50_000_000);
        // Now that we're below the maximum, we've gone too far:
        assert!(estimator.are_we_oom());
        allocate(40_000_000);
        assert!(estimator.are_we_oom());
    }

    // The intervals between checking if out-of-memory shrink as we get closer
    // to running out of memory
    #[test]
    fn oom_estimator_shrinking_intervals() {
        let (fake_memory, mut estimator) = setup_estimator();
        loop {
            {
                let mut memory = fake_memory.lock().unwrap();
                memory.allocate(10_000);
            }
            if estimator.too_big_allocation(10_000) {
                break;
            }
            // by 100MB we should have detected OOM.
            assert!(fake_memory.lock().unwrap().available_memory >= 99_000_000);
        }
        let fake_memory = fake_memory.lock().unwrap();
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
