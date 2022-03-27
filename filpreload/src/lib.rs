#![deny(unsafe_op_in_unsafe_fn)]
use parking_lot::Mutex;
use pymemprofile_api::memorytracking::{
    AllocationTracker, CallSiteId, Callstack, FunctionId, VecFunctionLocations, PARENT_PROCESS,
};
use pymemprofile_api::oom::{InfiniteMemory, OutOfMemoryEstimator, RealMemoryInfo};
use std::cell::RefCell;
use std::ffi::CStr;
use std::os::raw::{c_char, c_int, c_void};

#[macro_use]
extern crate lazy_static;

#[cfg(target_os = "linux")]
use tikv_jemallocator::Jemalloc;

#[cfg(target_os = "linux")]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;

thread_local!(static THREAD_CALLSTACK: RefCell<Callstack> = RefCell::new(Callstack::new()));

struct TrackerState {
    oom: OutOfMemoryEstimator,
    allocations: AllocationTracker<VecFunctionLocations>,
}

lazy_static! {
    static ref TRACKER_STATE: Mutex<TrackerState> = Mutex::new(TrackerState {
        allocations: AllocationTracker::new("/tmp".to_string(), VecFunctionLocations::new()),
        oom: OutOfMemoryEstimator::new(
            if std::env::var("__FIL_DISABLE_OOM_DETECTION") == Ok("1".to_string()) {
                Box::new(InfiniteMemory {})
            } else {
                Box::new(RealMemoryInfo::new())
            }
        ),
    });
}

/// Register a new function/filename location.
fn add_function(filename: String, function_name: String) -> FunctionId {
    let tracker_state = TRACKER_STATE.try_lock();
    if let Some(mut tracker_state) = tracker_state {
        tracker_state
            .allocations
            .functions
            .add_function(filename, function_name)
    } else {
        // This will help in SIGUSR2 handler: dumping calls into Python, we
        // can't really acquire lock since it's in the middle of dumping. So
        // just give up.
        FunctionId::UNKNOWN
    }
}

/// Add to per-thread function stack:
fn start_call(call_site: FunctionId, parent_line_number: u16, line_number: u16) {
    THREAD_CALLSTACK.with(|cs| {
        cs.borrow_mut()
            .start_call(parent_line_number, CallSiteId::new(call_site, line_number));
    });
}

/// Finish off (and move to reporting structure) current function in function
/// stack.
fn finish_call() {
    THREAD_CALLSTACK.with(|cs| {
        cs.borrow_mut().finish_call();
    });
}

/// Get the current thread's callstack.
fn get_current_callstack() -> Callstack {
    THREAD_CALLSTACK.with(|cs| (*cs.borrow()).clone())
}

/// Set the current callstack. Typically should only be used when starting up
/// new threads.
fn set_current_callstack(callstack: &Callstack) {
    THREAD_CALLSTACK.with(|cs| {
        *cs.borrow_mut() = callstack.clone();
    })
}

extern "C" {
    fn free(address: *mut c_void);
}

/// Add a new allocation based off the current callstack.
///
/// This can fail if the thread local with the Python stack is not available.
/// This only happens during thread exit where an allocation can sometimes be
/// triggered during thread-local cleanup for some reason.
fn add_allocation(
    address: usize,
    size: usize,
    line_number: u16,
    is_mmap: bool,
) -> Result<(), std::thread::AccessError> {
    let mut tracker_state = TRACKER_STATE.lock();
    let current_allocated_bytes = tracker_state.allocations.get_current_allocated_bytes();

    // Check if we're out of memory:
    let oom = (address == 0)
        || tracker_state
            .oom
            .too_big_allocation(size, current_allocated_bytes);

    // If we're out-of-memory, we're not going to exit this function or ever
    // free() anything ever again, so we should clear some memory in order to
    // reduce chances of running out as part of OOM reporting. We can also free
    // the allocation that just happened, cause it's never going to be used.
    if oom {
        if address == 0 {
            eprintln!(
                "=fil-profile= WARNING: Allocation of size {} failed (mmap()? {})",
                size, is_mmap
            );
        } else {
            unsafe {
                let address = address as *mut c_void;
                if is_mmap {
                    (pymemprofile_api::ffi::LIBC.munmap)(address, size);
                } else {
                    free(address);
                }
            }
        }
        tracker_state.allocations.oom_break_glass();
        eprintln!("=fil-profile= WARNING: Detected out-of-memory condition, exiting soon.");
        tracker_state.oom.print_info();
    }

    let allocations = &mut tracker_state.allocations;
    // Will fail during thread shutdown, but not much we can do at that point.
    let callstack_id = THREAD_CALLSTACK.try_with(|tcs| {
        let mut callstack = tcs.borrow_mut();
        callstack.id_for_new_allocation(line_number, |callstack| {
            allocations.get_callstack_id(callstack)
        })
    })?;

    if is_mmap {
        allocations.add_anon_mmap(PARENT_PROCESS, address, size, callstack_id);
    } else {
        allocations.add_allocation(PARENT_PROCESS, address, size, callstack_id);
    }

    if oom {
        // Uh-oh, we're out of memory.
        allocations.oom_dump();
    };
    Ok(())
}

/// Free an existing allocation.
fn free_allocation(address: usize) {
    let mut tracker_state = TRACKER_STATE.lock();

    let allocations = &mut tracker_state.allocations;
    allocations.free_allocation(PARENT_PROCESS, address);
}

/// Get the size of an allocation, or 0 if it's not tracked.
fn get_allocation_size(address: usize) -> usize {
    let tracker_state = TRACKER_STATE.lock();
    let allocations = &tracker_state.allocations;
    allocations.get_allocation_size(PARENT_PROCESS, address)
}

/// Reset internal state.
fn reset(default_path: String) {
    // Make sure we initialize this static, to prevent deadlocks:
    pymemprofile_api::ffi::initialize();
    let mut tracker_state = TRACKER_STATE.lock();
    tracker_state.allocations.reset(default_path);
}

/// Dump all callstacks in peak memory usage to format used by flamegraph.
fn dump_peak_to_flamegraph(path: &str) {
    let mut tracker_state = TRACKER_STATE.lock();
    let allocations = &mut tracker_state.allocations;
    allocations.dump_peak_to_flamegraph(path);
}

#[no_mangle]
extern "C" fn pymemprofile_add_allocation(address: usize, size: usize, line_number: u16) {
    add_allocation(address, size, line_number, false).unwrap_or(());
}

#[no_mangle]
extern "C" fn pymemprofile_free_allocation(address: usize) {
    free_allocation(address);
}

/// Returns allocation size, or 0 if not stored. Useful for tests, mostly.
#[no_mangle]
extern "C" fn pymemprofile_get_allocation_size(address: usize) -> usize {
    get_allocation_size(address)
}

#[no_mangle]
extern "C" fn pymemprofile_add_anon_mmap(address: usize, size: usize, line_number: u16) {
    add_allocation(address, size, line_number, true).unwrap_or(());
}

#[no_mangle]
unsafe extern "C" fn pymemprofile_add_function_location(
    filename: *const c_char,
    filename_length: u64,
    function_name: *const c_char,
    function_length: u64,
) -> u64 {
    let filename = unsafe {
        std::str::from_utf8_unchecked(std::slice::from_raw_parts(
            filename as *const u8,
            filename_length as usize,
        ))
    };
    let function_name = unsafe {
        std::str::from_utf8_unchecked(std::slice::from_raw_parts(
            function_name as *const u8,
            function_length as usize,
        ))
    };

    let function_id = add_function(filename.to_string(), function_name.to_string());
    function_id.as_u64()
}

/// # Safety
/// Intended for use from C APIs, what can I say.
#[no_mangle]
unsafe extern "C" fn pymemprofile_start_call(
    parent_line_number: u16,
    function_id: u64,
    line_number: u16,
) {
    let function_id = FunctionId::new(function_id);
    start_call(function_id, parent_line_number, line_number);
}

#[no_mangle]
extern "C" fn pymemprofile_finish_call() {
    finish_call();
}

/// # Safety
/// Intended for use from C.
#[no_mangle]
unsafe extern "C" fn pymemprofile_reset(default_path: *const c_char) {
    let path = unsafe { CStr::from_ptr(default_path) }
        .to_str()
        .expect("Path wasn't UTF-8")
        .to_string();
    reset(path);
}

/// # Safety
/// Intended for use from C.
#[no_mangle]
unsafe extern "C" fn pymemprofile_dump_peak_to_flamegraph(path: *const c_char) {
    let path = unsafe { CStr::from_ptr(path) }
        .to_str()
        .expect("Path wasn't UTF-8")
        .to_string();
    dump_peak_to_flamegraph(&path);
}

/// # Safety
/// Intended for use from C.
#[no_mangle]
unsafe extern "C" fn pymemprofile_get_current_callstack() -> *mut c_void {
    let callstack = get_current_callstack();
    let callstack = Box::new(callstack);
    Box::into_raw(callstack) as *mut c_void
}

/// # Safety
/// Intended for use from C.
#[no_mangle]
unsafe extern "C" fn pymemprofile_set_current_callstack(callstack: *mut c_void) {
    // The callstack is a Box created via pymemprofile_get_callstack()
    let callstack = unsafe { Box::<Callstack>::from_raw(callstack as *mut Callstack) };
    set_current_callstack(&callstack);
}

/// # Safety
/// Intended for use from C.
#[no_mangle]
unsafe extern "C" fn pymemprofile_clear_current_callstack() {
    let callstack = Callstack::new();
    set_current_callstack(&callstack);
}

/// # A start at implementing public API from Rust

/// Convert pointer into Rust closure.
extern "C" fn trampoline<F>(user_data: *mut c_void)
where
    F: FnMut(),
{
    let user_data = unsafe { &mut *(user_data as *mut F) };
    user_data();
}

/// C APIs in _filpreload.c.
type CCallback = extern "C" fn(*mut c_void);
extern "C" {
    // Call function conditonally in non-reentrant way.
    fn call_if_tracking(f: CCallback, user_data: *mut c_void) -> c_void;

    // Return whether C code has initialized.
    fn is_initialized() -> c_int;

    // Increment/decrement reentrancy counter.
    fn fil_increment_reentrancy();
    fn fil_decrement_reentrancy();
}

struct FilMmapAPI;

impl pymemprofile_api::mmap::MmapAPI for FilMmapAPI {
    fn call_if_tracking<F: FnMut()>(&self, mut f: F) {
        unsafe { call_if_tracking(trampoline::<F>, &mut f as *mut _ as *mut c_void) };
    }

    fn remove_mmap(&self, address: usize, length: usize) {
        let mut tracker_state = TRACKER_STATE.lock();

        let allocations = &mut tracker_state.allocations;
        allocations.free_anon_mmap(PARENT_PROCESS, address, length);
    }

    fn is_initialized(&self) -> bool {
        return unsafe { is_initialized() == 1 };
    }
}

/// On macOS we're using reimplemented_* prefix.
#[cfg(target_os = "macos")]
#[no_mangle]
pub extern "C" fn reimplemented_munmap(addr: *mut c_void, len: usize) -> c_int {
    return unsafe { pymemprofile_api::mmap::munmap_wrapper(addr, len, &FilMmapAPI {}) };
}

/// On Linux we're using same name as the API we're replacing.
#[cfg(target_os = "linux")]
#[no_mangle]
pub extern "C" fn munmap(addr: *mut c_void, len: usize) -> c_int {
    return unsafe { pymemprofile_api::mmap::munmap_wrapper(addr, len, &FilMmapAPI {}) };
}
