use im::hashmap as imhashmap;
use im::vector as imvector;
use libc;
use std::cell::RefCell;

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

struct Allocation {
    callstack: Callstack,
    size: libc::size_t,
}

static MEMORY_USAGE = imhashmap::HashMap<usize,Allocation>::new();

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
    let alloc = Allocation{callstack, size};
    MEMORY_USAGE.set(address, alloc);
}

/// Free an existing allocation.
pub fn free_allocation(address: usize) {}
