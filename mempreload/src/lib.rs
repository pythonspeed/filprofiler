use libloading::os::unix;
use std::ffi;
use static_alloc::Slab;

#[global_allocator]
static A: Slab<[u8; 1 << 16]> = Slab::uninit();

/// Do the necessary bookkeeping to update memory usage for current function on
/// stack.
/// TODO for current function in stack and all parents, maybe_set_new_peak().
fn update_memory_usage_while_in_malloc() {
    println!("UPDATING MEMORY USAGE!");
    return;/*
    if let Ok(lib) = unix::Library::open(Option::Some(ffi::OsStr::new("./target/debug/libpymemprofile_api.so")), libc::RTLD_DEEPBIND) {
        unsafe {
            if let Ok(symbol) = lib.get(b"update_memory_usage") {
                let func: unix::Symbol<unsafe extern fn()> = symbol;
                func();
            } else {
                panic!("Can't find update_memory_usage symbol");
            }
        }
    } else {
            panic!("Can't load libpymemprofile_api.so");
    }*/
}

// Override functions via C ABI, for LD_PRELOAD purposes.
// TODO: add realloc, posix_memalign.
redhook::hook! {
    unsafe fn malloc(size: libc::size_t) -> *mut libc::c_void => my_malloc {
        let result = redhook::real!(malloc)(size);
        update_memory_usage_while_in_malloc();
        result
    }
}

redhook::hook! {
    unsafe fn calloc(nmemb: libc::size_t, size: libc::size_t) -> *mut libc::c_void => my_calloc {
        let result = redhook::real!(calloc)(nmemb, size);
        update_memory_usage_while_in_malloc();
        result
    }
}

redhook::hook! {
    unsafe fn mmap(addr: *mut libc::c_void, length: libc::size_t, prot: libc::c_int, flags: libc::c_int, fd: libc::c_int, offset: libc::off_t) -> *mut libc::c_void => my_mmap {
        let result = redhook::real!(mmap)(addr, length, prot, flags, fd, offset);
        println!("MMAP! {}", flags & (libc::MAP_PRIVATE | libc::MAP_ANONYMOUS));
        if (flags & (libc::MAP_PRIVATE | libc::MAP_ANONYMOUS)) != 0 {
            // This suggests mmmap() used for allocation:
            update_memory_usage_while_in_malloc();
        }
        result
    }
}
