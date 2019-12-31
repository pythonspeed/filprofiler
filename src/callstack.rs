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
        Call{name, starting_memory, peak_memory: 0}
    }

    fn allocated_memory(&self) -> usize {
        if self.starting_memory > self.peak_memory {
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
                          starting_memory: 123, peak_memory: 0});
    assert_eq!(call.allocated_memory(), 0);
}

#[test]
fn call_updates_peak_if_higher_than_previous_peak() {
    let mut call = Call::new("mycall".to_string(), 123);
    call.update_memory_usage(100);
    assert_eq!(call, Call{name: "mycall".to_string(),
                          starting_memory: 123, peak_memory: 100});
    call.update_memory_usage(90);
    assert_eq!(call, Call{name: "mycall".to_string(),
                          starting_memory: 123, peak_memory: 100});
    call.update_memory_usage(101);
    assert_eq!(call, Call{name: "mycall".to_string(),
                          starting_memory: 123, peak_memory: 101});
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
        println!("{} {}", finished_call.callstack.join(";"), finished_call.allocated_by_call);
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
        let num_calls = self.calls.len();
        let baseline_memory = if num_calls > 0 {
            cmp::max(currently_used_memory, self.calls[num_calls-1].peak_memory)
        } else {
            currently_used_memory
        };
        self.calls.push(Call::new(name, baseline_memory));
    }

    fn finish_call(&mut self) {
        let call = self.calls.pop();
        match call {
            None => {
                println!("BUG, finished unstarted call?!");
            },
            Some(call) => {
                let mut callstack: Vec<String> = self.calls.iter().map(
                    |c| c.name.clone()).collect();
                callstack.push(call.name.clone());
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

thread_local!(static CALLSTACK: RefCell<Callstack> = RefCell::new(Callstack::new(Box::new(RecordToFile{}))));

/// Add to per-thread function stack:
pub fn start_call(name: String, currently_used_memory: usize) {
    CALLSTACK.with(|cs| {
        cs.borrow_mut().start_call(name, currently_used_memory);
    });
}

/// Finish off (and move to reporting structure) current function in function
/// stack.
pub fn finish_call() {
    CALLSTACK.with(|cs| {
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
