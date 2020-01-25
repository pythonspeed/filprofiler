use std::ffi::CStr;
use std::os::raw::c_char;

#[macro_use]
extern crate lazy_static;

mod memorytracking;

#[no_mangle]
pub extern "C" fn pymemprofile_add_allocation(address: usize, size: libc::size_t) {
    memorytracking::add_allocation(address, size);
}

#[no_mangle]
pub extern "C" fn pymemprofile_free_allocation(address: usize) {
    memorytracking::free_allocation(address);
}

#[no_mangle]
pub extern "C" fn pymemprofile_start_call(file_name: *const c_char, func_name: *const c_char) {
    let name = unsafe {
        format!(
            "{}:{}",
            CStr::from_ptr(file_name)
                .to_str()
                .expect("Function name wasn't UTF-8"),
            CStr::from_ptr(func_name)
                .to_str()
                .expect("Function name wasn't UTF-8")
        )
    };
    memorytracking::start_call(name);
}

#[no_mangle]
pub extern "C" fn pymemprofile_finish_call() {
    memorytracking::finish_call();
}

#[no_mangle]
pub extern "C" fn pymemprofile_reset() {
    memorytracking::reset();
}

#[no_mangle]
pub extern "C" fn pymemprofile_dump_peak_to_flamegraph(path: *const c_char) {
    let path = unsafe {
        CStr::from_ptr(path)
            .to_str()
            .expect("Path wasn't UTF-8")
            .to_string()
    };
    memorytracking::dump_peak_to_flamegraph(&path);
}

#[cfg(test)]
mod tests {}
