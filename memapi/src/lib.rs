use std::ffi::CStr;
use std::os::raw::c_char;

#[macro_use]
extern crate lazy_static;

#[cfg(target_os = "linux")]
use jemallocator::Jemalloc;

#[cfg(target_os = "linux")]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

mod memorytracking;
mod rangemap;

#[no_mangle]
pub extern "C" fn pymemprofile_add_allocation(
    address: usize,
    size: libc::size_t,
    line_number: u16,
) {
    memorytracking::add_allocation(address, size, line_number, false);
}

#[no_mangle]
pub extern "C" fn pymemprofile_free_allocation(address: usize) {
    memorytracking::free_allocation(address);
}

/// Returns allocation size, or 0 if not stored. Useful for tests, mostly.
#[no_mangle]
pub extern "C" fn pymemprofile_get_allocation_size(address: usize) -> libc::size_t {
    memorytracking::get_allocation_size(address)
}

#[no_mangle]
pub extern "C" fn pymemprofile_add_anon_mmap(address: usize, size: libc::size_t, line_number: u16) {
    memorytracking::add_allocation(address, size, line_number, true);
}

#[no_mangle]
pub extern "C" fn pymemprofile_free_anon_mmap(address: usize, length: libc::size_t) {
    memorytracking::free_anon_mmap(address, length);
}

/// # Safety
/// Intended for use from C APIs, what can I say.
#[no_mangle]
pub unsafe extern "C" fn pymemprofile_start_call(
    parent_line_number: u16,
    function_loc: *const memorytracking::FunctionLocation,
    line_number: u16,
) {
    let fid = memorytracking::FunctionId::new(function_loc);
    memorytracking::start_call(fid, parent_line_number, line_number);
}

#[no_mangle]
pub extern "C" fn pymemprofile_finish_call() {
    memorytracking::finish_call();
}

#[no_mangle]
pub extern "C" fn pymemprofile_new_line_number(line_number: u16) {
    memorytracking::new_line_number(line_number);
}

/// # Safety
/// Intended for use from C.
#[no_mangle]
pub unsafe extern "C" fn pymemprofile_reset(default_path: *const c_char) {
    let path = CStr::from_ptr(default_path)
        .to_str()
        .expect("Path wasn't UTF-8")
        .to_string();
    memorytracking::reset(path);
}

/// # Safety
/// Intended for use from C.
#[no_mangle]
pub unsafe extern "C" fn pymemprofile_dump_peak_to_flamegraph(path: *const c_char) {
    let path = CStr::from_ptr(path)
        .to_str()
        .expect("Path wasn't UTF-8")
        .to_string();
    memorytracking::dump_peak_to_flamegraph(&path);
}

#[cfg(test)]
mod tests {}
