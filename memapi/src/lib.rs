use crossbeam::atomic;
use std::cell::RefCell;
use std::ffi::CStr;
use std::os::raw::c_char;

#[macro_use]
extern crate lazy_static;

mod memorytracking;

thread_local!(static IN_THIS_LIBRARY: RefCell<bool> = RefCell::new(false));

static MAX_MEMORY: atomic::AtomicCell<usize> = atomic::AtomicCell::new(0);

/// Run the given function in such way that later calls to malloc() are handled
/// normally without being captured. Otherwise malloc() calls from this Rust
/// library trigger more Rust code, leading to infinite recursion, multiple
/// borrows, and other unpleasantness.
///
/// Basically: only call given function if we're being called directly from
/// outside world.
fn call_if_external_api(call: Box<dyn FnOnce() -> ()>) {
    IN_THIS_LIBRARY.with(|in_this_library| {
        if !*in_this_library.borrow() {
            *in_this_library.borrow_mut() = true;
            call();
            *in_this_library.borrow_mut() = false;
        }
    });
}

#[no_mangle]
pub extern "C" fn pymemprofile_add_allocation(address: usize, size: libc::size_t) {
    call_if_external_api(Box::new(move || {
        memorytracking::add_allocation(address, size);
    }));
}

#[no_mangle]
pub extern "C" fn pymemprofile_free_allocation(address: usize) {
    call_if_external_api(Box::new(move || {
        memorytracking::free_allocation(address);
    }));
}

#[no_mangle]
pub extern "C" fn pymemprofile_start_call(name: *const c_char) {
    let name = unsafe {
        CStr::from_ptr(name)
            .to_str()
            .expect("Function name wasn't UTF-8")
            .to_string()
    };
    call_if_external_api(Box::new(move || {
        memorytracking::start_call(name);
    }));
}

#[no_mangle]
pub extern "C" fn pymemprofile_finish_call() {
    call_if_external_api(Box::new(|| {
        memorytracking::finish_call();
    }));
}

#[no_mangle]
pub extern "C" fn pymemprofile_reset() {
    MAX_MEMORY.store(0);
    call_if_external_api(Box::new(|| {
        memorytracking::reset();
    }));
}

#[no_mangle]
pub extern "C" fn pymemprofile_dump_peak_to_flamegraph(path: *const c_char) {
    let path = unsafe {
        CStr::from_ptr(path)
            .to_str()
            .expect("Path wasn't UTF-8")
            .to_string()
    };
    call_if_external_api(Box::new(move || {
        memorytracking::dump_peak_to_flamegraph(&path);
        // TODO: Error handling?
    }));
}

#[cfg(test)]
mod tests {}
