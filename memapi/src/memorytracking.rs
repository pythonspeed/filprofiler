use im::hashmap as imhashmap;
use im::vector as imvector;
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
            "(N/A)".to_string()
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
    let alloc = Allocation { callstack, size };
    let mut allocations = ALLOCATIONS.lock().unwrap();
    allocations.current_allocations.insert(address, alloc);
    allocations.current_allocated_bytes += size;
    if allocations.current_allocated_bytes > allocations.peak_allocated_bytes {
        allocations.peak_allocated_bytes = allocations.current_allocated_bytes;
        allocations.peak_allocations = allocations.current_allocations.clone();
    }
}

/// Free an existing allocation.
pub fn free_allocation(address: usize) {
    let mut allocations = ALLOCATIONS.lock().unwrap();
    // Possibly this allocation doesn't exist; that's OK!
    if let Some(removed) = allocations.current_allocations.remove(&address) {
        if removed.size > allocations.current_allocated_bytes {
            allocations.current_allocated_bytes = 0;
        } else {
            allocations.current_allocated_bytes -= removed.size;
        }
    }
}

/// Reset internal state.
pub fn reset() {
    *ALLOCATIONS.lock().unwrap() = AllocationTracker::new();
}

/// Dump all callstacks in peak memory usage to format used by flamegraph.
pub fn dump_peak_to_flamegraph(_path: &str) {
    // Convert to mapping from callstack to usage, merging usage for duplicate
    // callstacks:
    let mut by_call: collections::HashMap<String, usize> = collections::HashMap::new();
    for Allocation { callstack, size } in ALLOCATIONS.lock().unwrap().peak_allocations.values() {
        let callstack = callstack.to_string();
        let entry = by_call.entry(callstack).or_insert(0);
        *entry += size;
    }
    for (callstack, size) in by_call.iter() {
        println!("{} {}", callstack, size);
    }
}
