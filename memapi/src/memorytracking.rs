use im::hashmap as imhashmap;
use im::vector as imvector;
use inferno::flamegraph;
use itertools::Itertools;
use libc;
use std::cell::RefCell;
use std::collections;
use std::sync::Mutex;

#[derive(Clone)]
struct Callstack {
    calls: imvector::Vector<String>,
}

impl Callstack {
    fn new() -> Callstack {
        Callstack {
            calls: imvector::Vector::<String>::new(),
        }
    }

    fn start_call(&mut self, name: String) {
        self.calls.push_back(name);
    }

    fn finish_call(&mut self) {
        self.calls.pop_back();
    }

    fn to_string(&self) -> String {
        if self.calls.len() == 0 {
            "[No Python stack]".to_string()
        } else {
            self.calls.iter().join(";")
        }
    }
}

thread_local!(static THREAD_CALLSTACK: RefCell<Callstack> = RefCell::new(Callstack::new()));

#[derive(Clone)]
struct Allocation {
    callstack: Callstack,
    size: libc::size_t,
}

struct AllocationTracker {
    current_allocations: imhashmap::HashMap<usize, Allocation>,
    peak_allocations: imhashmap::HashMap<usize, Allocation>,
    current_allocated_bytes: usize,
    peak_allocated_bytes: usize,
}

impl AllocationTracker {
    fn new() -> AllocationTracker {
        AllocationTracker {
            current_allocations: imhashmap::HashMap::new(),
            peak_allocations: imhashmap::HashMap::new(),
            current_allocated_bytes: 0,
            peak_allocated_bytes: 0,
        }
    }

    /// Add a new allocation based off the current callstack.
    fn add_allocation(&mut self, address: usize, size: libc::size_t, callstack: Callstack) {
        let alloc = Allocation { callstack, size };
        self.current_allocations.insert(address, alloc);
        self.current_allocated_bytes += size;
        if self.current_allocated_bytes > self.peak_allocated_bytes {
            self.peak_allocated_bytes = self.current_allocated_bytes;
            self.peak_allocations = self.current_allocations.clone();
        }
    }

    /// Free an existing allocation.
    fn free_allocation(&mut self, address: usize) {
        // Possibly this allocation doesn't exist; that's OK!
        if let Some(removed) = self.current_allocations.remove(&address) {
            if removed.size > self.current_allocated_bytes {
                // In theory this should never happen, but just in case...
                self.current_allocated_bytes = 0;
            } else {
                self.current_allocated_bytes -= removed.size;
            }
        }
    }

    /// Dump all callstacks in peak memory usage to format used by flamegraph.
    fn dump_peak_to_flamegraph(&self, path: &str) {
        // Convert to mapping from callstack to usage, merging usage for duplicate
        // callstacks:
        let mut by_call: collections::HashMap<String, usize> = collections::HashMap::new();
        let peak_allocations = &self.peak_allocations;
        for Allocation { callstack, size } in peak_allocations.values() {
            let callstack = callstack.to_string();
            let entry = by_call.entry(callstack).or_insert(0);
            *entry += size;
        }
        let lines: Vec<String> = by_call
            .iter()
            .map(|(callstack, size)| {
                format!("{} {:.0}", callstack, (*size as f64 / 1024.0).round())
            })
            .collect();
        match write_flamegraph(
            lines.iter().map(|s| s.as_ref()),
            path,
            self.peak_allocated_bytes,
        ) {
            Ok(_) => {
                eprintln!("Wrote memory usage flamegraph to {}", path);
            }
            Err(e) => {
                eprintln!("Error writing SVG: {}", e);
            }
        }
    }
}

lazy_static! {
    static ref ALLOCATIONS: Mutex<AllocationTracker> = Mutex::new(AllocationTracker::new());
}

/// Add to per-thread function stack:
pub fn start_call(name: String) {
    THREAD_CALLSTACK.with(|cs| {
        cs.borrow_mut().start_call(name);
    });
}

/// Finish off (and move to reporting structure) current function in function
/// stack.
pub fn finish_call() {
    THREAD_CALLSTACK.with(|cs| {
        cs.borrow_mut().finish_call();
    });
}

/// Add a new allocation based off the current callstack.
pub fn add_allocation(address: usize, size: libc::size_t) {
    let callstack: Callstack = THREAD_CALLSTACK.with(|cs| (*cs.borrow()).clone());
    let mut allocations = ALLOCATIONS.lock().unwrap();
    allocations.add_allocation(address, size, callstack);
}

/// Free an existing allocation.
pub fn free_allocation(address: usize) {
    let mut allocations = ALLOCATIONS.lock().unwrap();
    allocations.free_allocation(address);
}

/// Reset internal state.
pub fn reset() {
    *ALLOCATIONS.lock().unwrap() = AllocationTracker::new();
}

/// Dump all callstacks in peak memory usage to format used by flamegraph.
pub fn dump_peak_to_flamegraph(path: &str) {
    let allocations = &ALLOCATIONS.lock().unwrap();
    allocations.dump_peak_to_flamegraph(path);
}

fn write_flamegraph<'a, I: IntoIterator<Item = &'a str>>(
    lines: I,
    path: &str,
    peak_bytes: usize,
) -> std::io::Result<()> {
    let file = std::fs::File::create(path)?;
    let title = format!(
        "Peak Tracked Memory Usage ({:.1} MiB)",
        peak_bytes as f64 / (1024.0 * 1024.0)
    );
    let mut options = flamegraph::Options {
        title: title,
        direction: flamegraph::Direction::Inverted,
        count_name: "KiB".to_string(),
        colors: flamegraph::color::Palette::Basic(flamegraph::color::BasicPalette::Mem),
        font_size: 16,
        font_type: "mono".to_string(),
        frame_height: 22,
        hash: true,
        ..Default::default()
    };
    if let Err(e) = flamegraph::from_lines(&mut options, lines, file) {
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("{}", e),
        ))
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{AllocationTracker, Callstack};
    use itertools::Itertools;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn current_allocated_matches_sum_of_allocations(
            // Allocated bytes. Will use index as the memory address.
            allocated_sizes in prop::collection::vec(1..1000 as usize, 10..20),
            // Allocations to free.
            free_indices in prop::collection::vec(any::<prop::sample::Index>(), 5..10)
        ) {
            let mut tracker = AllocationTracker::new();
            for i in 0..allocated_sizes.len() {
                tracker.add_allocation(i as usize,*allocated_sizes.get(i).unwrap(), Callstack::new());
            }
            let mut expected_sum = allocated_sizes.iter().sum();
            prop_assert_eq!(tracker.current_allocated_bytes, expected_sum);
            for i in free_indices.iter().map(|i|i.index(allocated_sizes.len())).unique() {
                expected_sum -= allocated_sizes.get(i).unwrap();
                tracker.free_allocation(i);
                prop_assert_eq!(tracker.current_allocated_bytes, expected_sum);
            }
        }
    }
}
