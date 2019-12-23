use std::cell::RefCell;
use sysinfo::{ProcessExt, SystemExt};

thread_local!(static IN_MALLOC: RefCell<bool> = RefCell::new(false));

fn get_memory_usage() -> u64 {
    let mut system = sysinfo::System::new_with_specifics(sysinfo::RefreshKind::new());
    let result = sysinfo::get_current_pid();
    match result {
        Ok(pid) => {
            system.refresh_process(pid);
            let optional_process = system.get_process(pid);
            match optional_process {
                None => {
                    println!("Couldn't find current process?! This is a bug.");
                    std::process::exit(1)
                },
                Some(process) => process.memory(),
            }
        },
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
                get_memory_usage();
                *in_malloc.borrow_mut() = false;
            }
        });
        result
    }
}
