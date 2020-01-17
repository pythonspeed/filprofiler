use im::hashmap as imhashmap;
use im::vector as imvector;
use itertools::Itertools;
use libc;
use std::cell::RefCell;
use std::collections;
use std::sync::Arc;
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

lazy_static! {
    static ref MEMORY_USAGE: Mutex<imhashmap::HashMap<usize, Allocation>> =
        Mutex::new(imhashmap::HashMap::new());
    static ref PEAK_MEMORY_USAGE: Mutex<imhashmap::HashMap<usize, Allocation>> =
        Mutex::new(imhashmap::HashMap::new());
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
    let mut map = MEMORY_USAGE.lock().unwrap();
    map.insert(address, alloc);
}

/// Free an existing allocation.
pub fn free_allocation(address: usize) {
    let mut map = MEMORY_USAGE.lock().unwrap();
    // Possibly this allocation doesn't exist; that's OK!
    map.remove(&address);
}

/// A new peak usage has been reached. Record current allocations for
/// (potential) dumping to flamegraph if it turns out this is the global peak.
pub fn new_peak() {
    *PEAK_MEMORY_USAGE.lock().unwrap() = MEMORY_USAGE.lock().unwrap().clone();
}

/// Dump all callstacks in peak memory usage to format used by flamegraph.
pub fn dump_peak_to_flamegraph(_path: &str) {
    // Convert to mapping from callstack to usage, merging usage for duplicate
    // callstacks:
    let mut by_call: collections::HashMap<String, usize> = collections::HashMap::new();
    for Allocation { callstack, size } in PEAK_MEMORY_USAGE.lock().unwrap().values() {
        let callstack = callstack.to_string();
        let entry = by_call.entry(callstack).or_insert(0);
        *entry += size;
    }
    for (callstack, size) in by_call.iter() {
        println!("{} {}", callstack, size);
    }
}
