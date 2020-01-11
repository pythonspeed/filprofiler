use im::hashmap as imhashmap;
use im::vector as imvector;
use libc;
use std::cell::RefCell;

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
}

thread_local!(static THREAD_CALLSTACK: RefCell<Callstack> = RefCell::new(Callstack::new()));

#[derive(Clone)]
struct Allocation {
    callstack: Callstack,
    size: libc::size_t,
}

static MEMORY_USAGE: Arc<Mutex<imhashmap::HashMap<usize, Allocation>>> =
    Arc::new(Mutex::new(imhashmap::HashMap::new()));

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
