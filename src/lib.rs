use std::cell::RefCell;
//use sysinfo::{ProcessExt, SystemExt};

thread_local!(static IN_MALLOC: RefCell<bool> = RefCell::new(false));

// procinfo-based is much much faster, but Linux-only for now.
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

redhook::hook! {
    unsafe fn malloc(size: libc::size_t) -> *mut libc::c_void => my_malloc {
        let result = redhook::real!(malloc)(size);
        IN_MALLOC.with(|in_malloc| {
            // If we're in malloc() already, this is recursive call from some
            // Rust code, and we don't want to do intercept logic when this
            // library's Rust calls malloc().
            if !*in_malloc.borrow() {
                *in_malloc.borrow_mut() = true;
                println!("ALLOCATED: {}", get_memory_usage());
                *in_malloc.borrow_mut() = false;
            }
        });
        result
    }
}
