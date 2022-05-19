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

/// # Safety
/// Only call with pointers from mmap()!
pub unsafe fn munmap_wrapper<A: MmapAPI>(addr: *mut c_void, len: usize, api: &A) -> c_int {
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

#[cfg(test)]
mod tests {
    use super::{munmap_wrapper, MmapAPI};
    use crate::ffi::LIBC;

    struct TrackMunmap {
        tracking_removed: std::cell::Cell<(usize, usize)>,
    }

    impl MmapAPI for TrackMunmap {
        fn is_initialized(&self) -> bool {
            true
        }
        fn call_if_tracking<F: FnMut()>(&self, mut f: F) {
            f()
        }
        fn remove_mmap(&self, addr: usize, len: usize) {
            // The map should still exist at this point!
            assert!(exists_in_maps(addr, len));
            self.tracking_removed.set((addr, len));
        }
    }

    // Return whether given mmap() exists for this process.
    fn exists_in_maps(addr: usize, len: usize) -> bool {
        for map in proc_maps::get_process_maps(std::process::id() as proc_maps::Pid).unwrap() {
            if map.start() == addr && map.size() >= len {
                return true;
            }
        }
        false
    }
    // Removing tracking metadata after munmap() can lead to race conditions if
    // another thread mmap()s the same address.
    #[test]
    fn munmap_happens_after_metadata_removal() {
        let size = 3072;
        let addr = unsafe {
            (LIBC.mmap)(
                std::ptr::null_mut(),
                size,
                libc::PROT_READ,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                -1,
                0,
            )
        };
        assert!(exists_in_maps(addr as usize, size));
        let fake_api = TrackMunmap {
            tracking_removed: std::cell::Cell::new((0, 0)),
        };
        unsafe { munmap_wrapper(addr, size, &fake_api) };
        assert_eq!(fake_api.tracking_removed.get(), (addr as usize, size));
        assert!(!exists_in_maps(addr as usize, size));
    }
}
