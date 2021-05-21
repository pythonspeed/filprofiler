// Calls into libc.
use libc::{c_int, c_void, off64_t, size_t};
use libloading::os::unix::{Library, Symbol};
use once_cell::sync::Lazy;

type Mmap = unsafe extern "C" fn(
    addr: *mut c_void,
    len: size_t,
    prot: c_int,
    flags: c_int,
    fd: c_int,
    offset: off64_t,
) -> *mut c_void;

type Munmap = unsafe extern "C" fn(addr: *mut c_void, length: usize) -> c_int;

/// Calls into glibc.
#[cfg(target_os = "linux")]
pub struct Libc {
    _library: Library,
    pub mmap: Symbol<Mmap>,
    pub munmap: Symbol<Munmap>,
}

#[cfg(target_os = "linux")]
pub static LIBC: Lazy<Libc> = Lazy::new(|| unsafe {
    let library = Library::new("libc.so.6").unwrap();
    let mmap = library.get(b"mmap64").unwrap();
    let munmap = library.get(b"munmap").unwrap();
    Libc {
        _library: library,
        mmap,
        munmap,
    }
});

#[cfg(target_os = "macos")]
pub struct Libc {
    pub mmap: Mmap,
    pub munmap: Munmap,
}

#[cfg(target_os = "macos")]
pub static LIBC: Lazy<Libc> = Lazy::new(|| unsafe {
    Libc {
        mmap: libc::mmap64,
        munmap: libc::munmap,
    }
});

// We're only loading thread-safe libc APIs.
unsafe impl Send for Libc {}
unsafe impl Sync for Libc {}

/// Initialize the Lazy static.
pub fn initialize() {
    Lazy::force(&LIBC);
}
