use std::cell::RefCell;
use std::cmp;

/// A function call in Python (or other languages wrapping this library).
/// Memory usage is in bytes.
#[derive(PartialEq,Eq,Debug)]
struct Call {
    name: String,
    starting_memory: usize,
    peak_memory: usize,
}

impl Call {
    fn new(name: String, starting_memory: usize) -> Call {
        Call{name, starting_memory, peak_memory: starting_memory}
    }

    fn allocated_memory(&self) -> usize {
        if self.starting_memory > self.peak_memory {
            // Technically should never happen, but just in case:
            0
        } else {
            self.peak_memory - self.starting_memory
        }
    }

    fn update_memory_usage(&mut self, currently_used_memory: usize) {
        if currently_used_memory > self.peak_memory {
            self.peak_memory = currently_used_memory;
        }
    }
}

#[test]
fn call_no_allocated_memory() {
    let mut call = Call::new("mycall".to_string(), 123);
    assert_eq!(call, Call{name: "mycall".to_string(),
                          starting_memory: 123, peak_memory: 123});
    assert_eq!(call.allocated_memory(), 0);
}

#[test]
fn call_updates_peak_if_higher_than_previous_peak() {
    let mut call = Call::new("mycall".to_string(), 123);
    call.update_memory_usage(200);
    assert_eq!(call, Call{name: "mycall".to_string(),
                          starting_memory: 123, peak_memory: 200});
    call.update_memory_usage(200);
    assert_eq!(call, Call{name: "mycall".to_string(),
                          starting_memory: 123, peak_memory: 200});
    call.update_memory_usage(201);
    assert_eq!(call, Call{name: "mycall".to_string(),
                          starting_memory: 123, peak_memory: 201});
}

#[test]
fn call_allocated_memory() {
    let mut call = Call::new("mycall".to_string(), 123);
    call.update_memory_usage(137);
    assert_eq!(call.allocated_memory(), 14);
    call.update_memory_usage(139);
    assert_eq!(call.allocated_memory(), 16);
}

/// A finished call.
struct FinishedCall {
    // TODO: switch to immutable datastructure to reduce copying?
    callstack: Vec<String>,
    allocated_by_call: usize,
}

/// Record finished calls.
trait RecordFinishedCall {
    fn record(&mut self, finished_call: FinishedCall) {}
}

struct RecordToMemory {
    finished_calls: Vec<FinishedCall>,
}

impl RecordToMemory {
    fn new() -> RecordToMemory {
        RecordToMemory{finished_calls: Vec::new()}
    }
}

impl RecordFinishedCall for RecordToMemory {
    fn record(&mut self, finished_call: FinishedCall) {
        self.finished_calls.push(finished_call);
    }
}

struct RecordToFile {
    // TODO
}

impl RecordFinishedCall for RecordToFile {
    fn record(&mut self, finished_call: FinishedCall) {
        if finished_call.allocated_by_call > 0 {
            println!("{} {}", finished_call.callstack.join(";"), finished_call.allocated_by_call / 1000000);
        }
    }
}
/// A callstack.
struct Callstack {
    calls: Vec<Call>,
    recorder: Box<dyn RecordFinishedCall>,
}

impl Callstack {
    fn new(recorder: Box<dyn RecordFinishedCall>) -> Callstack {
        Callstack{calls: Vec::new(), recorder}
    }

    fn start_call(&mut self, name: String, currently_used_memory: usize) {
        // TODO maybe update_memory_usage() with new value?
        let num_calls = self.calls.len();
        let baseline_memory = if num_calls > 0 {
            cmp::max(currently_used_memory, self.calls[num_calls-1].peak_memory)
        } else {
            currently_used_memory
        };
        self.calls.push(Call::new(name, baseline_memory));
    }

    fn current_calls(&self) -> Vec<String> {
        self.calls.iter().map(|c| c.name.clone()).collect()
    }

    // For testing.
    fn current_allocated(&self) -> Vec<usize> {
        self.calls.iter().map(|c| c.allocated_memory()).collect()
    }

    fn finish_call(&mut self) {
        let callstack: Vec<String> = self.current_calls();
        let call = self.calls.pop();
        match call {
            None => {
                panic!("I was asked to finish a call, but the callstack is empty!");
            },
            Some(call) => {
                let allocated_by_call = call.allocated_memory();
                let finished_call = FinishedCall{callstack, allocated_by_call};
                self.recorder.record(finished_call);
            },
        }
    }

    fn update_memory_usage(&mut self, currently_used_memory: usize) {
        for call in self.calls.iter_mut() {
            &call.update_memory_usage(currently_used_memory);
        }
    }
}

#[test]
fn callstack_update_memory_usage_updates_full_stack() {
    let mut callstack = Callstack::new(Box::new(RecordToMemory::new()));
    callstack.start_call("a".to_string(), 2);
    callstack.start_call("b".to_string(), 2);
    callstack.update_memory_usage(10);
    assert_eq!(callstack.current_calls(), ["a", "b"]);
    assert_eq!(callstack.current_allocated(), [8, 8]);
    // Memory baseline that is less than current peak of 10:
    callstack.start_call("c".to_string(), 10);
    assert_eq!(callstack.current_calls(), ["a", "b", "c"]);
    assert_eq!(callstack.current_allocated(), [8, 8, 0]);
    callstack.start_call("d".to_string(), 10);
    callstack.update_memory_usage(15);
    assert_eq!(callstack.current_calls(), ["a", "b", "c", "d"]);
    assert_eq!(callstack.current_allocated(), [13, 13, 5, 5]);
}

#[test]
fn callstack_start_call_starting_memory_at_least_previous_peak() {
    let mut callstack = Callstack::new(Box::new(RecordToMemory::new()));
    callstack.start_call("a".to_string(), 2);
    callstack.start_call("b".to_string(), 1);
    assert_eq!(callstack.calls[1].starting_memory, 2);
    callstack.start_call("c".to_string(), 3);
    assert_eq!(callstack.calls[1].starting_memory, 2);
    assert_eq!(callstack.calls[2].starting_memory, 3);
}

// fn callstack_finish_call...

thread_local!(static CALLSTACK: RefCell<Callstack> = RefCell::new(Callstack::new(Box::new(RecordToFile{}))));

/// Add to per-thread function stack:
pub fn start_call(name: String, currently_used_memory: usize) {
    CALLSTACK.with(|cs| {
        println!("start call {} {}", name, currently_used_memory / 1000000);
        cs.borrow_mut().start_call(name, currently_used_memory);
    });
}

/// Finish off (and move to reporting structure) current function in function
/// stack.
pub fn finish_call() {
    CALLSTACK.with(|cs| {
        println!("finish call");
        cs.borrow_mut().finish_call();
    });
}

/// Update memory usage for calls in stack:
pub fn update_memory_usage(currently_used_memory: usize) {
    CALLSTACK.with(|cs| {
        cs.borrow_mut().update_memory_usage(currently_used_memory);
    });
}
/// Create flamegraph SVG from function stack:
pub fn dump_functions_to_flamegraph_svg(path: String) {
}
