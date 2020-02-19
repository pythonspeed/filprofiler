use std::ffi::CStr;
use std::os::raw::c_char;
use std::str;

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

/// # Safety
/// Intended for use from C APIs, what can I say.
#[no_mangle]
pub unsafe extern "C" fn pymemprofile_start_call(
    file_name: *const c_char,
    func_name: *const c_char,
) {
    let function_name = str::from_utf8_unchecked(CStr::from_ptr(func_name).to_bytes());
    let module_name = str::from_utf8_unchecked(CStr::from_ptr(file_name).to_bytes());
    let call_site = memorytracking::CallSite::new(module_name, function_name);
    memorytracking::start_call(call_site);
}

#[no_mangle]
pub extern "C" fn pymemprofile_finish_call() {
    memorytracking::finish_call();
}

#[no_mangle]
pub extern "C" fn pymemprofile_reset() {
    memorytracking::reset();
}

/// # Safety
/// Intended for use from C APIs, what can I say.
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
