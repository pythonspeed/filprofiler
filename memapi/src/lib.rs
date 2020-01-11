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

// Return current process memory usage. procinfo-based is much much faster than
// sysinfo-based, but Linux-only for now.
fn get_memory_usage() -> usize {
    let result = procinfo::pid::statm_self();
    match result {
        Ok(statm) => statm.resident * page_size::get(),
        Err(_) => {
            println!("Couldn't find current process?! This is a bug.");
            std::process::exit(1)
        }
    }
}

#[no_mangle]
pub extern "C" fn pymemprofile_add_allocation(address: usize, size: libc::size_t) {
    call_if_external_api(Box::new(move || {
        memorytracking::add_allocation(address, size);
        // Technically not thread-safe, but not a big deal if we mark peak
        // twice. The failure mode is peak will be slightly smaller than it
        // actually was, which means higher peaks will still get noticed, so
        // that's OK too.
        let current_memory = get_memory_usage();
        if current_memory > MAX_MEMORY.load() {
            MAX_MEMORY.store(current_memory);
            memorytracking::new_peak();
        }
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
