use std::cell::RefCell;
//use sysinfo::{ProcessExt, SystemExt};

thread_local!(static IN_MALLOC: RefCell<bool> = RefCell::new(false));

// Return current process memory usage. procinfo-based is much much faster than
// sysinfo-based, but Linux-only for now.
fn get_memory_usage() -> usize {
    let result = procinfo::pid::statm_self();
    match result {
        Ok(statm) => statm.resident * page_size::get(),
        Err(E) => {
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
            println!("Memory usage: {}", get_memory_usage());
            *in_malloc.borrow_mut() = false;
        }
    });

}

/// Override functions via C ABI, for LD_PRELOAD purposes.
// TODO: add calloc, realloc, posix_memalign. Probably not mmap?
redhook::hook! {
    unsafe fn malloc(size: libc::size_t) -> *mut libc::c_void => my_malloc {
        let result = redhook::real!(malloc)(size);
        update_memory_usage_while_in_malloc();
        result
    }
}

// TODO name for "python function" that is more generic?
// TODO expose next three via C ABI for use by Python

/// Add to per-thread function stack:
fn start_function(name: str) {
}

/// Finish off (and move to reporting structure) current function in function
/// stack.
fn finish_function() {
}

/// Create flamegraph SVG from function stack:
fn dump_functions_to_flamegraph_svg() {
}
