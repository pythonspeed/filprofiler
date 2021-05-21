use super::ffi::LIBC;
/// mmap API business logic.
use std::os::raw::{c_int, c_void};

/// Need to use pattern here: https://stackoverflow.com/a/37608197/6214034

pub trait MmapAPI {
    /// Call if we're not reentrant.
    fn call_if_tracking<F: FnMut()>(&self, f: F);

    /// Implement removal of tracking metdata.
    fn remove_mmap(&self, addr: usize, len: usize);

    /// Return whether C module is initialized.
    fn is_initialized(&self) -> bool;
}

pub fn munmap_wrapper<A: MmapAPI>(addr: *mut c_void, len: usize, api: A) -> c_int {
    if !api.is_initialized() {
        #[cfg(target_os = "macos")]
        {
            return unsafe { libc::munmap(addr, len) };
        };
        #[cfg(target_os = "linux")]
        {
            return unsafe { libc::syscall(libc::SYS_munmap, addr, len) } as c_int;
        }
    }
    api.call_if_tracking(|| {
        api.remove_mmap(addr as usize, len);
    });
    // If munmap() fails the above removal is wrong, but that's highly unlikley
    // to happen, and we want to prevent a threading race condition so need to
    // remove tracking metdata first.
    unsafe { (LIBC.munmap)(addr, len) }
}
