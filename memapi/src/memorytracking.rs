use inferno::flamegraph;
use itertools::Itertools;
use libc;
use rustc_hash;
use smallstr::SmallString;
use smallvec::SmallVec;
use std::cell::RefCell;
use std::collections;
use std::fmt;
use std::sync::Mutex;

#[derive(Clone, Debug, PartialEq)]
struct Callstack {
    calls: Vec<u32>,
}

impl Callstack {
    fn new() -> Callstack {
        Callstack { calls: Vec::new() }
    }

    fn start_call(&mut self, function_id: u32) {
        self.calls.push(function_id);
    }

    fn finish_call(&mut self) {
        self.calls.pop();
    }

    fn as_string(&self, call_sites: &CallSites) -> String {
        if self.calls.is_empty() {
            "[No Python stack]".to_string()
        } else {
            self.calls
                .iter()
                .map(|id| call_sites.get_callsite(*id))
                .join(";")
        }
    }
}

thread_local!(static THREAD_CALLSTACK: RefCell<Callstack> = RefCell::new(Callstack::new()));

/// A particular place where a call happened:
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct CallSite {
    pub module_name: String,   //SmallString<[u8; 24]>,
    pub function_name: String, //SmallString<[u8; 24]>,
}

impl fmt::Display for CallSite {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}:{}", self.module_name, self.function_name)
    }
}
/// Maps CallSites to integer identifiers used in CallStacks.
struct CallSites {
    max_id: u32,
    callsite_to_id: rustc_hash::FxHashMap<CallSite, u32>,
    //id_to_callsite: rustc_hash::FxHashMap<u32, CallSite>,
}

impl CallSites {
    fn new() -> CallSites {
        CallSites {
            max_id: 0,
            callsite_to_id: rustc_hash::FxHashMap::default(),
            //id_to_callsite: rustc_hash::FxHashMap::default(),
        }
    }

    fn get_or_insert_id(&mut self, call_site: CallSite) -> u32 {
        let max_id = &mut self.max_id;
        let result = self.callsite_to_id.entry(call_site).or_insert_with(|| {
            let result = *max_id;
            *max_id += 1;
            result
        });
        *result
    }

    fn get_callsite(&self, id: u32) -> CallSite {
        // TODO this is super-slow, precalculate reverse map
        for (call_site, csid) in &(self.callsite_to_id) {
            if *csid == id {
                return call_site.clone();
            }
        }
        panic!("ono")
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Allocation {
    callstack: Callstack,
    size: libc::size_t,
}

struct AllocationTracker {
    current_allocations: rustc_hash::FxHashMap<usize, Allocation>,
    peak_allocations: rustc_hash::FxHashMap<usize, Allocation>,
    current_allocated_bytes: usize,
    peak_allocated_bytes: usize,
    call_sites: CallSites,
}

impl<'a> AllocationTracker {
    fn new() -> AllocationTracker {
        AllocationTracker {
            current_allocations: rustc_hash::FxHashMap::default(),
            peak_allocations: rustc_hash::FxHashMap::default(),
            current_allocated_bytes: 0,
            peak_allocated_bytes: 0,
            call_sites: CallSites::new(),
        }
    }

    /// Add a new allocation based off the current callstack.
    fn add_allocation(&mut self, address: usize, size: libc::size_t, callstack: Callstack) {
        let alloc = Allocation { callstack, size };
        self.current_allocations.insert(address, alloc);
        self.current_allocated_bytes += size;
        if self.current_allocated_bytes > self.peak_allocated_bytes + 10000 {
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

    /// Combine Callstacks and make them human-readable. Duplicate callstacks
    /// have their allocated memory summed.
    fn combine_callstacks(&self) -> collections::HashMap<String, usize> {
        let mut by_call: collections::HashMap<String, usize> = collections::HashMap::new();
        let peak_allocations = &self.peak_allocations;
        for Allocation { callstack, size } in peak_allocations.values() {
            let callstack = callstack.as_string(&self.call_sites);
            let entry = by_call.entry(callstack).or_insert(0);
            *entry += size;
        }
        by_call
    }

    /// Dump all callstacks in peak memory usage to format used by flamegraph.
    fn dump_peak_to_flamegraph(&self, path: &str) {
        let by_call = self.combine_callstacks();
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
pub fn start_call(call_site: CallSite) {
    let mut allocations = ALLOCATIONS.lock().unwrap();
    let id = allocations.call_sites.get_or_insert_id(call_site);
    THREAD_CALLSTACK.with(|cs| {
        cs.borrow_mut().start_call(id);
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
        title,
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
    use std::collections;

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

    #[test]
    fn peak_allocations_only_updated_on_new_peaks() {
        let mut tracker = AllocationTracker::new();
        let mut cs1 = Callstack::new();
        cs1.start_call(1);
        let mut cs2 = Callstack::new();
        cs2.start_call(2);

        tracker.add_allocation(1, 1000, cs1.clone());
        // Peak should now match current allocations:
        assert_eq!(tracker.current_allocations, tracker.peak_allocations);
        assert_eq!(tracker.peak_allocated_bytes, 1000);
        let previous_peak = tracker.peak_allocations.clone();

        // Free the allocation:
        tracker.free_allocation(1);
        assert_eq!(tracker.current_allocated_bytes, 0);
        assert_eq!(previous_peak, tracker.peak_allocations);
        assert_eq!(tracker.peak_allocated_bytes, 1000);

        // Add allocation, still less than 1000:
        tracker.add_allocation(3, 123, cs1.clone());
        assert_eq!(previous_peak, tracker.peak_allocations);
        assert_eq!(tracker.peak_allocated_bytes, 1000);

        // Add allocation that goes past previous peak
        tracker.add_allocation(2, 2000, cs2.clone());
        assert_eq!(tracker.current_allocations, tracker.peak_allocations);
        assert_eq!(tracker.peak_allocated_bytes, 2123);
    }

    #[test]
    fn combine_callstacks_and_sum_allocations() {
        let mut tracker = AllocationTracker::new();
        let mut cs1 = Callstack::new();
        cs1.start_call(1);
        cs1.start_call(2);
        let mut cs2 = Callstack::new();
        cs2.start_call(3);

        tracker.add_allocation(1, 1000, cs1.clone());
        tracker.add_allocation(2, 234, cs2.clone());
        tracker.add_allocation(3, 50000, cs1.clone());

        let mut expected: collections::HashMap<String, usize> = collections::HashMap::new();
        expected.insert("a;b".to_string(), 51000);
        expected.insert("c".to_string(), 234);
        assert_eq!(expected, tracker.combine_callstacks());
    }
}
