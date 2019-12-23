use std::cell::RefCell;

/// A function call in Python (or other languages wrapping this library).
/// Memory usage is in bytes.
struct Call {
    name: String,
    starting_memory: u64,
    peak_memory: u64,
}

impl Call {
    fn new(name: String, starting_memory: u64) -> Call {
        Call{name, starting_memory, peak_memory: 0}
    }

    fn allocated_memory(&self) -> u64 {
        if self.starting_memory > self.peak_memory {
            0
        } else {
            self.peak_memory - self.starting_memory
        }
    }

    fn update_memory_usage(&mut self, currently_used_memory: u64) {
        if currently_used_memory > self.peak_memory {
            self.peak_memory = currently_used_memory;
        }
    }
}

/// A callstack.
struct Callstack {
    calls: Vec<Call>,
}

impl Callstack {
    fn new() -> Callstack {
        Callstack{calls: Vec::new()}
    }

    fn start_call(&mut self, name: String, currently_used_memory: u64) {
        self.calls.push(Call::new(name, currently_used_memory));
    }

    fn finish_call(&mut self) {
        let call = self.calls.pop();
        match call {
            None => {
                println!("BUG, finished unstarted call?!");
            },
            Some(call) => {
                println!("TODO call finished, log it somehow: {} {}", call.name, call.allocated_memory());
            },
        }
    }

    fn update_memory_usage(&mut self, currently_used_memory: u64) {
        for call in &mut self.calls {
            call.update_memory_usage(currently_used_memory);
        }
    }
}

thread_local!(static CALLSTACK: RefCell<Callstack> = RefCell::new(Callstack::new()));

/// Add to per-thread function stack:
fn start_call(name: String, currently_used_memory: u64) {
    CALLSTACK.with(|cs| {
        cs.borrow_mut().start_call(name, currently_used_memory);
    });
}

/// Finish off (and move to reporting structure) current function in function
/// stack.
fn finish_call() {
    CALLSTACK.with(|cs| {
        cs.borrow_mut().finish_call();
    });
}

/// Update memory usage for calls in stack:
fn update_memory_usage(currently_used_memory: u64) {
    CALLSTACK.with(|cs| {
        cs.borrow_mut().update_memory_usage(currently_used_memory);
    });
}
/// Create flamegraph SVG from function stack:
fn dump_functions_to_flamegraph_svg() {
}
