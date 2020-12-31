/// Logic for handling out-of-memory situations.

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
///
/// If current process is only one allocating, that is fine, it'll catch the
/// situation. But there might be multiple processes allocating. So separately
/// we'll also check every millisecond in a thread. That way if we're running
/// out of memory due to something else, we'll still dump and not lose the info.
/// That's implemented elsewhere. TODO
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
        // Anything less than this is too dangerous, and we should dump and
        // exit. TODO is this actually enough?
        const MINIMAL_FREE: usize = 100 * 1024 * 1024;

        // Figure out how much is free, reset the threshold accordingly.
        let available_bytes = (self.get_available_bytes)();

        // Check if we're out of memory:
        if available_bytes < MINIMAL_FREE {
            return true;
        }

        // Still have enough, so threshold to 1% of distance to danger zone.
        self.check_threshold_bytes = (available_bytes - MINIMAL_FREE) / 100;

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

/// Return how much free memory we have, as bytes.
pub fn get_available_memory() -> usize {
    // TODO cgroups
    // This will include memory that can become available by syncing
    // filesystem buffers to disk, which is probably what we want.
    let available = psutil::memory::virtual_memory().unwrap().available() as usize;
    available
}
