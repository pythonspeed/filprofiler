use std::thread;
use std::cell::RefCell;

thread_local!(static IN_MALLOC: RefCell<bool> = RefCell::new(false));

redhook::hook! {
    unsafe fn malloc(size: libc::size_t) -> *mut libc::c_void => my_malloc {
        let result = redhook::real!(malloc)(size);
        IN_MALLOC.with(|in_malloc| {
            // If we're in malloc() already, this is recursive call from some
            // Rust code, and we don't want to do intercept logic when this
            // library's Rust calls malloc().
            if (!*in_malloc.borrow()) {
                *in_malloc.borrow_mut() = true;
                println!("MALLOCED!");
                *in_malloc.borrow_mut() = false;
            }
        });
        result
    }
}
