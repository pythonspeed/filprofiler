use std::cell::RefCell;
use std::ffi::CStr;
use std::os::raw::c_char;

mod callstack;

thread_local!(static IN_MALLOC: RefCell<bool> = RefCell::new(false));

// Return current process memory usage. procinfo-based is much much faster than
// sysinfo-based, but Linux-only for now.
fn get_memory_usage() -> usize {
    let result = procinfo::pid::statm_self();
    match result {
        Ok(statm) => statm.resident * page_size::get(),
        Err(_) => {
            println!("Couldn't find current process?! This is a bug.");
            std::process::exit(1)
        },
    }
}

/// Do the necessary bookkeeping to update memory usage for current function on
/// stack.
/// TODO for current function in stack and all parents, maybe_set_new_peak().
fn update_memory_usage_while_in_malloc() {
    IN_MALLOC.with(|in_malloc| {
        // If we're in malloc() already, this is recursive call from some
        // Rust code, and we don't want to do intercept logic when this
        // library's Rust calls malloc().
        if !*in_malloc.borrow() {
            *in_malloc.borrow_mut() = true;
            let memory = get_memory_usage();
            println!("Memory usage: {}", memory);
            callstack::update_memory_usage(memory);
            *in_malloc.borrow_mut() = false;
        }
    });

}

// Override functions via C ABI, for LD_PRELOAD purposes.
// TODO: add calloc, realloc, posix_memalign. Probably not mmap?
redhook::hook! {
    unsafe fn malloc(size: libc::size_t) -> *mut libc::c_void => my_malloc {
        let result = redhook::real!(malloc)(size);
        update_memory_usage_while_in_malloc();
        result
    }
}

#[no_mangle]
pub extern "C" fn pymemprofile_start_call(name: *const c_char) {
    let name = unsafe {
        CStr::from_ptr(name).to_str().expect(
            "Function name wasn't UTF-8")
    };
    callstack::start_call(name.to_string(), get_memory_usage());
}

#[no_mangle]
pub extern "C" fn pymemprofile_finish_call() {
    callstack::finish_call();
}

#[no_mangle]
pub extern "C" fn pymemprofile_dump_functions_to_flamegraph_svg(path: *const c_char) {
    let path = unsafe {
        CStr::from_ptr(path).to_str().expect("Path wasn't UTF-8")
    };
    callstack::dump_functions_to_flamegraph_svg(path.to_string());
    // TODO: Error handling?
}
