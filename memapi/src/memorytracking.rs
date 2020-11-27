use super::rangemap::RangeMap;
use ahash::AHashMap as HashMap;
use im::Vector as ImVector;
use inferno::flamegraph;
use itertools::Itertools;
use libc;
use parking_lot::Mutex;
use std::cell::RefCell;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::path::PathBuf;
use std::slice;

/// A function location provided by the C code. Matches struct in _filpreload.c.
#[repr(C)]
pub struct FunctionLocation {
    filename: *const u8,
    filename_length: isize,
    function_name: *const u8,
    function_name_length: isize,
}

impl FunctionLocation {
    #[cfg(test)]
    fn from_strings(filename: &str, function_name: &str) -> Self {
        FunctionLocation {
            filename: filename.as_ptr(),
            filename_length: filename.len() as isize,
            function_name: function_name.as_ptr(),
            function_name_length: function_name.len() as isize,
        }
    }
}

/// A Rust-y wrapper for FunctionLocation
#[derive(Clone, Debug, PartialEq, Eq, Copy, Hash)]
pub struct FunctionId {
    function: *const FunctionLocation,
}

unsafe impl Send for FunctionId {}
unsafe impl Sync for FunctionId {}

impl FunctionId {
    pub fn new(function: *const FunctionLocation) -> Self {
        FunctionId { function }
    }

    fn get_filename(&self) -> &str {
        unsafe {
            let loc = &*self.function;
            let slice = slice::from_raw_parts(loc.filename, loc.filename_length as usize);
            std::str::from_utf8_unchecked(slice)
        }
    }

    fn get_function_name(&self) -> &str {
        unsafe {
            let loc = &*self.function;
            let slice = slice::from_raw_parts(loc.function_name, loc.function_name_length as usize);
            std::str::from_utf8_unchecked(slice)
        }
    }
}

/// A specific location: file + function + line number.
#[derive(Clone, Debug, PartialEq, Eq, Copy, Hash)]
struct CallSiteId {
    function: FunctionId,
    /// Line number within the _file_, 1-indexed.
    line_number: u16,
}

impl CallSiteId {
    fn new(function: FunctionId, line_number: u16) -> CallSiteId {
        CallSiteId {
            function,
            line_number,
        }
    }
}

/// The current Python callstack. We use IDs instead of Function objects for
/// performance reasons.
#[derive(Derivative)]
#[derivative(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Callstack {
    calls: Vec<CallSiteId>,
    #[derivative(Hash = "ignore", PartialEq = "ignore")]
    cached_callstack_id: Option<(u16, CallstackId)>, // first bit is line number
}

impl Callstack {
    pub fn new() -> Callstack {
        Callstack {
            calls: Vec::new(),
            cached_callstack_id: None,
        }
    }

    fn start_call(&mut self, parent_line_number: u16, callsite_id: CallSiteId) {
        if parent_line_number != 0 {
            if let Some(mut call) = self.calls.last_mut() {
                call.line_number = parent_line_number;
            }
        }
        self.calls.push(callsite_id);
        self.cached_callstack_id = None;
    }

    fn finish_call(&mut self) {
        self.calls.pop();
        self.cached_callstack_id = None;
    }

    fn id_for_new_allocation<F>(&mut self, line_number: u16, get_callstack_id: F) -> CallstackId
    where
        F: FnOnce(&Callstack) -> CallstackId,
    {
        // If same line number as last callstack, and we have cached callstack
        // ID, reuse it:
        if let Some((previous_line_number, callstack_id)) = self.cached_callstack_id {
            if line_number == previous_line_number {
                return callstack_id;
            }
        }

        // Set the new line number:
        if line_number != 0 {
            if let Some(call) = self.calls.last_mut() {
                call.line_number = line_number;
            }
        }

        // Calculate callstack ID, cache it, and then return it;
        let callstack_id = get_callstack_id(self);
        self.cached_callstack_id = Some((line_number, callstack_id));
        callstack_id
    }

    fn as_string(&self, to_be_post_processed: bool) -> String {
        if self.calls.is_empty() {
            "[No Python stack]".to_string()
        } else {
            self.calls
                .iter()
                .map(|id| {
                    if to_be_post_processed {
                        format!(
                            "{filename}:{line} ({function});TB@@{filename}:{line}@@TB",
                            filename = id.function.get_filename(),
                            line = id.line_number,
                            function = id.function.get_function_name(),
                        )
                    } else {
                        format!(
                            "{filename}:{line} ({function})",
                            filename = id.function.get_filename(),
                            line = id.line_number,
                            function = id.function.get_function_name()
                        )
                    }
                })
                .join(";")
        }
    }
}

thread_local!(static THREAD_CALLSTACK: RefCell<Callstack> = RefCell::new(Callstack::new()));

type CallstackId = u32;

/// Maps Functions to integer identifiers used in CallStacks.
struct CallstackInterner {
    max_id: CallstackId,
    callstack_to_id: HashMap<Callstack, u32>,
}

impl<'a> CallstackInterner {
    fn new() -> Self {
        CallstackInterner {
            max_id: 0,
            callstack_to_id: HashMap::default(),
        }
    }

    /// Add a (possibly) new Function, returning its ID.
    fn get_or_insert_id<F: FnOnce() -> ()>(
        &mut self,
        callstack: &Callstack,
        call_on_new: F,
    ) -> CallstackId {
        let max_id = &mut self.max_id;
        if let Some(result) = self.callstack_to_id.get(callstack) {
            *result
        } else {
            let new_id = *max_id;
            *max_id += 1;
            self.callstack_to_id.insert(callstack.clone(), new_id);
            call_on_new();
            new_id
        }
    }

    /// Get map from IDs to Functions.
    fn get_reverse_map(&self) -> HashMap<CallstackId, &Callstack> {
        let mut result = HashMap::default();
        for (call_site, csid) in self.callstack_to_id.iter() {
            result.insert(*csid, call_site);
        }
        result
    }
}

const MIB: usize = 1024 * 1024;
const HIGH_32BIT: u32 = 1 << 31;

/// A specific call to malloc()/calloc().
#[derive(Clone, Copy, Debug, PartialEq)]
struct Allocation {
    callstack_id: CallstackId,
    // If high bit is set, this is MiBs (without the high bit being meaningful).
    // Otherwise, it's bytes. We only store MiBs for allocations larger than 2
    // ** 31 bytes (2GB), which means the loss of resolution isn't meaningful.
    // This compression allows us to reduce memory overhead from tracking
    // allocations.
    compressed_size: u32,
}

impl Allocation {
    fn new(callstack_id: CallstackId, size: libc::size_t) -> Self {
        let compressed_size = if size >= HIGH_32BIT as usize {
            // Rounding division by MiB, plus the high bit:
            (((size + MIB / 2) / MIB) as u32) | HIGH_32BIT
        } else {
            size as u32
        };
        Allocation {
            callstack_id,
            compressed_size,
        }
    }

    fn size(&self) -> libc::size_t {
        if self.compressed_size >= HIGH_32BIT {
            (self.compressed_size - HIGH_32BIT) as libc::size_t * MIB
        } else {
            self.compressed_size as libc::size_t
        }
    }
}

// Filter down to top 99% of allocated memory.
//
// 1. Empty callstacks are dropped.
// 2. Top 99% of allocations, starting with largest, are kept.
// 3. If that's less than 100 allocations, thrown in up to 100, main goal is
//    just to not have a vast number of useless tiny allocations.
fn filter_to_useful_callstacks(allocations: &ImVector<usize>) -> HashMap<CallstackId, usize> {
    let total_allocated: usize = allocations.iter().sum();
    let mut stored = 0.0;
    allocations
        .iter()
        // Convert to (callstack id, size) tuples:
        .enumerate()
        // Filter out callstacks with no allocations:
        .filter(|(_, size)| **size > 0)
        // Sort in descending size of allocation:
        .sorted_by(|a, b| Ord::cmp(b.1, a.1))
        // Keep track of how much total allocations we've accumulated so far:
        .map(|(i, size)| {
            stored += *size as f64;
            (stored.clone(), i as u32, size)
        })
        // Stop once we've hit 99% of allocations, but include at least 100 just
        // so there's some context:
        .scan((false, 0), |(past_threshold, taken), (stored, i, size)| {
            if *past_threshold && (*taken > 99) {
                return None;
            }
            *past_threshold = (stored / total_allocated as f64) >= 0.99;
            *taken += 1;
            Some((i, *size))
        })
        .collect()
}

/// The main data structure tracking everything.
struct AllocationTracker {
    // malloc()/calloc():
    current_allocations: HashMap<usize, Allocation>,
    // anonymous mmap(), i.e. not file backed:
    current_anon_mmaps: RangeMap<CallstackId>,

    // Map CallstackIds to Callstacks, so we can store the former and save
    // memory:
    interner: CallstackInterner,

    // Both malloc() and mmap():
    current_memory_usage: ImVector<usize>, // Map CallstackId -> total memory usage
    peak_memory_usage: ImVector<usize>,    // Map CallstackId -> total memory usage
    current_allocated_bytes: usize,
    peak_allocated_bytes: usize,
    // Default directory to write out data lacking other info:
    default_path: String,
}

impl<'a> AllocationTracker {
    fn new(default_path: String) -> AllocationTracker {
        AllocationTracker {
            current_allocations: HashMap::default(),
            current_anon_mmaps: RangeMap::new(),
            interner: CallstackInterner::new(),
            current_memory_usage: ImVector::new(),
            peak_memory_usage: ImVector::new(),
            current_allocated_bytes: 0,
            peak_allocated_bytes: 0,
            default_path,
        }
    }

    /// Check if a new peak has been reached:
    fn check_if_new_peak(&mut self) {
        if self.current_allocated_bytes > self.peak_allocated_bytes {
            self.peak_allocated_bytes = self.current_allocated_bytes;
            self.peak_memory_usage
                .clone_from(&self.current_memory_usage);
        }
    }

    fn add_memory_usage(&mut self, callstack_id: CallstackId, bytes: usize) {
        self.current_allocated_bytes += bytes;
        let index = callstack_id as usize;
        self.current_memory_usage[index] += bytes;
    }

    fn remove_memory_usage(&mut self, callstack_id: CallstackId, bytes: usize) {
        self.current_allocated_bytes -= bytes;
        let index = callstack_id as usize;
        // TODO what if goes below zero? add a check I guess, in case of bugs.
        self.current_memory_usage[index] -= bytes;
    }

    fn get_callstack_id(&mut self, callstack: &Callstack) -> CallstackId {
        let current_memory_usage = &mut self.current_memory_usage;
        self.interner
            .get_or_insert_id(callstack, || current_memory_usage.push_back(0))
    }

    /// Add a new allocation based off the current callstack.
    fn add_allocation(&mut self, address: usize, size: libc::size_t, callstack_id: CallstackId) {
        let alloc = Allocation::new(callstack_id, size);
        let compressed_size = alloc.size();
        self.current_allocations.insert(address, alloc);
        self.add_memory_usage(callstack_id, compressed_size as usize);
    }

    /// Free an existing allocation.
    fn free_allocation(&mut self, address: usize) {
        // Before we reduce memory, let's check if we've previously hit a peak:
        self.check_if_new_peak();

        // Possibly this allocation doesn't exist; that's OK! It can if e.g. we
        // didn't capture an allocation for some reason.
        if let Some(removed) = self.current_allocations.remove(&address) {
            self.remove_memory_usage(removed.callstack_id, removed.size());
        }
    }

    /// Add a new anonymous mmap() based of the current callstack.
    fn add_anon_mmap(&mut self, address: usize, size: libc::size_t, callstack_id: CallstackId) {
        self.current_anon_mmaps.add(address, size, callstack_id);
        self.add_memory_usage(callstack_id, size);
    }

    fn free_anon_mmap(&mut self, address: usize, size: libc::size_t) {
        // Before we reduce memory, let's check if we've previously hit a peak:
        self.check_if_new_peak();
        // Now remove, and update totoal memory tracking:
        for (callstack_id, removed) in self.current_anon_mmaps.remove(address, size) {
            self.remove_memory_usage(callstack_id, removed);
        }
    }

    /// Combine Callstacks and make them human-readable. Duplicate callstacks
    /// have their allocated memory summed.
    fn combine_callstacks(
        &mut self,
        // If false, will do the current allocations:
        peak: bool,
    ) -> HashMap<CallstackId, usize> {
        // First, make sure peaks are correct:
        self.check_if_new_peak();

        // We get a LOT of tiny allocations. To reduce overhead of creating
        // flamegraph (which currently loads EVERYTHING into memory), just do
        // the top 99% of allocations.
        if peak {
            filter_to_useful_callstacks(&self.peak_memory_usage)
        } else {
            filter_to_useful_callstacks(&self.current_memory_usage)
        }
    }

    /// Dump all callstacks in peak memory usage to various files describing the
    /// memory usage.
    fn dump_peak_to_flamegraph(&mut self, path: &str) {
        self.dump_to_flamegraph(path, true, "peak-memory", "Peak Tracked Memory Usage", true);
    }

    fn to_lines(
        &mut self,
        peak: bool,
        to_be_post_processed: bool,
    ) -> impl Iterator<Item = String> + '_ {
        let by_call = self.combine_callstacks(peak).into_iter();
        let id_to_callstack = self.interner.get_reverse_map();
        by_call.map(move |(callstack_id, size)| {
            format!(
                "{} {}",
                id_to_callstack
                    .get(&callstack_id)
                    .unwrap()
                    .as_string(to_be_post_processed),
                size,
            )
        })
    }

    fn dump_to_flamegraph(
        &mut self,
        path: &str,
        peak: bool,
        base_filename: &str,
        title: &str,
        to_be_post_processed: bool,
    ) {
        eprintln!("=fil-profile= Preparing to write to {}", path);
        let directory_path = Path::new(path);

        if !directory_path.exists() {
            fs::create_dir_all(directory_path)
                .expect("=fil-profile= Couldn't create the output directory.");
        } else if !directory_path.is_dir() {
            panic!("=fil-profile= Output path must be a directory.");
        }

        let raw_path = directory_path
            .join(format!("{}.prof", base_filename))
            .to_str()
            .unwrap()
            .to_string();

        if let Err(e) = write_lines(self.to_lines(peak, to_be_post_processed), &raw_path) {
            eprintln!("=fil-profile= Error writing raw profiling data: {}", e);
        }
        let svg_path = directory_path
            .join(format!("{}.svg", base_filename))
            .to_str()
            .unwrap()
            .to_string();
        match write_flamegraph(
            &raw_path,
            &svg_path,
            self.peak_allocated_bytes,
            false,
            title,
            to_be_post_processed,
        ) {
            Ok(_) => {
                eprintln!(
                    "=fil-profile= Wrote memory usage flamegraph to {}",
                    svg_path
                );
            }
            Err(e) => {
                eprintln!("=fil-profile= Error writing SVG: {}", e);
            }
        }
        let svg_path = directory_path
            .join(format!("{}-reversed.svg", base_filename))
            .to_str()
            .unwrap()
            .to_string();
        match write_flamegraph(
            &raw_path,
            &svg_path,
            self.peak_allocated_bytes,
            true,
            title,
            to_be_post_processed,
        ) {
            Ok(_) => {
                eprintln!(
                    "=fil-profile= Wrote memory usage flamegraph to {}",
                    svg_path
                );
            }
            Err(e) => {
                eprintln!("=fil-profile= Error writing SVG: {}", e);
            }
        }
    }

    /// Dump information about where we are.
    fn oom_dump(&mut self) {
        eprintln!("=fil-profile= Uh oh, out of memory!");
        eprintln!(
            "=fil-profile= We'll try to dump out SVGs. Note that no HTML file will be written."
        );
        let default_path = self.default_path.clone();
        self.dump_to_flamegraph(
            &default_path,
            false,
            "out-of-memory",
            "Current allocations at out-of-memory time",
            false,
        );
        unsafe {
            libc::_exit(5);
        }
    }
}

lazy_static! {
    static ref ALLOCATIONS: Mutex<AllocationTracker> =
        Mutex::new(AllocationTracker::new("/tmp".to_string()));
}

/// Add to per-thread function stack:
pub fn start_call(call_site: FunctionId, parent_line_number: u16, line_number: u16) {
    THREAD_CALLSTACK.with(|cs| {
        cs.borrow_mut()
            .start_call(parent_line_number, CallSiteId::new(call_site, line_number));
    });
}

/// Finish off (and move to reporting structure) current function in function
/// stack.
pub fn finish_call() {
    THREAD_CALLSTACK.with(|cs| {
        cs.borrow_mut().finish_call();
    });
}

/// Get the current thread's callstack.
pub fn get_current_callstack() -> Callstack {
    THREAD_CALLSTACK.with(|cs| (*cs.borrow()).clone())
}

/// Set the current callstack. Typically should only be used when starting up
/// new threads.
pub fn set_current_callstack(callstack: &Callstack) {
    THREAD_CALLSTACK.with(|cs| {
        *cs.borrow_mut() = callstack.clone();
    })
}

/// Add a new allocation based off the current callstack.
pub fn add_allocation(address: usize, size: libc::size_t, line_number: u16, is_mmap: bool) {
    let mut allocations = ALLOCATIONS.lock();
    let callstack_id = THREAD_CALLSTACK.with(|tcs| {
        let mut callstack = tcs.borrow_mut();
        callstack.id_for_new_allocation(line_number, |callstack| {
            allocations.get_callstack_id(callstack)
        })
    });

    if is_mmap {
        allocations.add_anon_mmap(address, size, callstack_id);
    } else {
        allocations.add_allocation(address, size, callstack_id);
    }

    if address == 0 {
        // Uh-oh, we're out of memory.
        allocations.oom_dump();
    }
}

/// Free an existing allocation.
pub fn free_allocation(address: usize) {
    let mut allocations = ALLOCATIONS.lock();
    allocations.free_allocation(address);
}

/// Get the size of an allocation, or 0 if it's not tracked.
pub fn get_allocation_size(address: usize) -> libc::size_t {
    let allocations = ALLOCATIONS.lock();
    if let Some(allocation) = allocations.current_allocations.get(&address) {
        allocation.size()
    } else {
        0
    }
}

/// Free an anonymous mmap().
pub fn free_anon_mmap(address: usize, length: libc::size_t) {
    let mut allocations = ALLOCATIONS.lock();
    allocations.free_anon_mmap(address, length);
}

/// Reset internal state.
pub fn reset(default_path: String) {
    *ALLOCATIONS.lock() = AllocationTracker::new(default_path);
}

/// Dump all callstacks in peak memory usage to format used by flamegraph.
pub fn dump_peak_to_flamegraph(path: &str) {
    let mut allocations = ALLOCATIONS.lock();
    allocations.dump_peak_to_flamegraph(path);
}

/// Write strings to disk, one line per string.
fn write_lines<I: Iterator<Item = String>>(lines: I, path: &str) -> std::io::Result<()> {
    let mut file = fs::File::create(path)?;
    for line in lines {
        file.write_all(line.as_bytes())?;
        file.write_all(b"\n")?;
    }
    file.flush()?;
    Ok(())
}

/// Write a flamegraph SVG to disk, given lines in summarized format.
fn write_flamegraph(
    lines_file_path: &str,
    path: &str,
    peak_bytes: usize,
    reversed: bool,
    title: &str,
    to_be_post_processed: bool,
) -> std::io::Result<()> {
    let mut file = std::fs::File::create(path)?;
    let title = format!(
        "{}{} ({:.1} MiB)",
        title,
        if reversed { ", Reversed" } else { "" },
        peak_bytes as f64 / (1024.0 * 1024.0)
    );
    let mut options = flamegraph::Options {
        title,
        count_name: "bytes".to_string(),
        font_size: 16,
        font_type: "mono".to_string(),
        frame_height: 22,
        reverse_stack_order: reversed,
        color_diffusion: true,
        direction: flamegraph::Direction::Inverted,
        // Maybe disable this some day, but for now it makes debugging much
        // easier:
        pretty_xml: true,
        ..Default::default()
    };
    if to_be_post_processed {
        options.subtitle = Some("SUBTITLE-HERE".to_string());
    }
    if let Err(e) = flamegraph::from_files(&mut options, &[PathBuf::from(lines_file_path)], &file) {
        Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("{}", e),
        ))
    } else {
        file.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        filter_to_useful_callstacks, Allocation, AllocationTracker, CallSiteId, Callstack,
        CallstackInterner, FunctionId, FunctionLocation, HIGH_32BIT, MIB,
    };
    use ahash::AHashMap as HashMap;
    use im;
    use proptest::prelude::*;

    proptest! {
        // Allocation sizes smaller than 2 ** 31 are round-tripped.
        #[test]
        fn small_allocation(size in 0..(HIGH_32BIT - 1)) {
            let allocation = Allocation::new(0, size as usize);
            prop_assert_eq!(size as usize, allocation.size());
        }

        // Allocation sizes larger than 2 ** 31 are stored as MiBs, with some
        // loss of resolution.
        #[test]
        fn large_allocation(size in (HIGH_32BIT as usize)..(1 << 50)) {
            let allocation = Allocation::new(0, size as usize);
            let result_size = allocation.size();
            let diff = if size < result_size {
                result_size - size
            } else {
                size - result_size
            };
            prop_assert!(diff <= MIB / 2)
        }

        // Test for https://github.com/pythonspeed/filprofiler/issues/66
        #[test]
        fn correct_allocation_size_tracked(size in (1 as usize)..(1<< 50)) {
            let mut tracker = AllocationTracker::new(".".to_string());
            let cs_id = tracker.get_callstack_id(&Callstack::new());
            tracker.add_allocation(0, size, cs_id);
            tracker.add_anon_mmap(1, size * 2, cs_id);
            // We don't track (large) allocations exactly right, but they should
            // be quite close:
            let ratio = ((size * 3) as f64) / (tracker.current_memory_usage[0] as f64);
            prop_assert!(0.999 < ratio);
            prop_assert!(ratio < 1.001);
            tracker.free_allocation(0);
            tracker.free_anon_mmap(1, size * 2);
            // Once we've freed everything, it should be _exactly_ 0.
            prop_assert_eq!(&im::vector![0], &tracker.current_memory_usage);
        }

        #[test]
        fn current_allocated_matches_sum_of_allocations(
            // Allocated bytes. Will use index as the memory address.
            allocated_sizes in prop::collection::vec(1..1000 as usize, 10..20),
            // Allocations to free.
            free_indices in prop::collection::btree_set(0..10 as usize, 1..5)
        ) {
            let mut tracker = AllocationTracker::new(".".to_string());
            let mut expected_memory_usage = im::vector![];
            for i in 0..allocated_sizes.len() {
                let mut cs = Callstack::new();
                cs.start_call(0, CallSiteId::new(FunctionId::new(i as *const FunctionLocation), 0));
                let cs_id = tracker.get_callstack_id(&cs);
                tracker.add_allocation(i as usize,*allocated_sizes.get(i).unwrap(), cs_id);
                expected_memory_usage.push_back(*allocated_sizes.get(i).unwrap());
            }
            let mut expected_sum = allocated_sizes.iter().sum();
            let expected_peak : usize = expected_sum;
            prop_assert_eq!(tracker.current_allocated_bytes, expected_sum);
            prop_assert_eq!(&tracker.current_memory_usage, &expected_memory_usage);
            for i in free_indices.iter() {
                expected_sum -= allocated_sizes.get(*i).unwrap();
                tracker.free_allocation(*i);
                expected_memory_usage[*i] -= allocated_sizes.get(*i).unwrap();
                prop_assert_eq!(tracker.current_allocated_bytes, expected_sum);
                prop_assert_eq!(&tracker.current_memory_usage, &expected_memory_usage);
            }
            prop_assert_eq!(tracker.peak_allocated_bytes, expected_peak);
        }

        #[test]
        fn current_allocated_anon_maps_matches_sum_of_allocations(
            // Allocated bytes. Will use index as the memory address.
            allocated_sizes in prop::collection::vec(1..1000 as usize, 10..20),
            // Allocations to free.
            free_indices in prop::collection::btree_set(0..10 as usize, 1..5)
        ) {
            let mut tracker = AllocationTracker::new(".".to_string());
            let mut expected_memory_usage = im::vector![];
            // Make sure addresses don't overlap:
            let addresses : Vec<usize> = (0..allocated_sizes.len()).map(|i| i * 10000).collect();
            for i in 0..allocated_sizes.len() {
                let mut cs = Callstack::new();
                cs.start_call(0, CallSiteId::new(FunctionId::new(i as *const FunctionLocation), 0));
                let csid = tracker.get_callstack_id(&cs);
                tracker.add_anon_mmap(addresses[i] as usize, *allocated_sizes.get(i).unwrap(), csid);
                expected_memory_usage.push_back(*allocated_sizes.get(i).unwrap());
            }
            let mut expected_sum = allocated_sizes.iter().sum();
            let expected_peak : usize = expected_sum;
            prop_assert_eq!(tracker.current_allocated_bytes, expected_sum);
            prop_assert_eq!(&tracker.current_memory_usage, &expected_memory_usage);
            for i in free_indices.iter() {
                expected_sum -= allocated_sizes.get(*i).unwrap();
                tracker.free_anon_mmap(addresses[*i], *allocated_sizes.get(*i).unwrap());
                expected_memory_usage[*i] -= allocated_sizes.get(*i).unwrap();
                prop_assert_eq!(tracker.current_allocated_bytes, expected_sum);
                prop_assert_eq!(&tracker.current_memory_usage, &expected_memory_usage);
            }
            prop_assert_eq!(tracker.peak_allocated_bytes, expected_peak);
        }

        #[test]
        fn filtering_of_callstacks(
            // Allocated bytes. Will use index as the memory address.
            allocated_sizes in prop::collection::vec(1..1000 as usize, 5..5000),
        ) {
            let total_size : usize = allocated_sizes.iter().sum();
            let filtered = filter_to_useful_callstacks(&im::Vector::from(allocated_sizes));
            let filtered_size = filtered.values().into_iter().sum::<usize>() as f64;
            prop_assert!(filtered_size >= 0.99 * (total_size as f64));
            if filtered.len() > 100 {
                // Removing any item should take us below 99%
                for value in filtered.values() {
                    prop_assert!(filtered_size - (*value as f64) < 0.99 * (total_size as f64));
                }
            }
        }
    }

    #[test]
    fn functionlocation_and_functionid_strings() {
        let func = FunctionLocation::from_strings("a", "af");
        let fid = FunctionId::new(&func as *const FunctionLocation);
        assert_eq!(fid.get_filename(), "a");
        assert_eq!(fid.get_function_name(), "af");
    }

    #[test]
    fn callstack_line_numbers() {
        let func1 = FunctionLocation::from_strings("a", "af");
        let func3 = FunctionLocation::from_strings("b", "bf");
        let func5 = FunctionLocation::from_strings("c", "cf");

        let fid1 = FunctionId::new(&func1 as *const FunctionLocation);
        let fid3 = FunctionId::new(&func3 as *const FunctionLocation);
        let fid5 = FunctionId::new(&func5 as *const FunctionLocation);

        // Parent line number does nothing if it's first call:
        let mut cs1 = Callstack::new();
        let id1 = CallSiteId::new(fid1, 2);
        let id2 = CallSiteId::new(fid3, 45);
        let id3 = CallSiteId::new(fid5, 6);
        cs1.start_call(123, id1);
        assert_eq!(cs1.calls, vec![id1]);

        // Parent line number does nothing if it's 0:
        cs1.start_call(0, id2);
        assert_eq!(cs1.calls, vec![id1, id2]);

        // Parent line number overrides previous level if it's non-0:
        let mut cs2 = Callstack::new();
        cs2.start_call(0, id1);
        cs2.start_call(10, id2);
        cs2.start_call(12, id3);
        assert_eq!(
            cs2.calls,
            vec![CallSiteId::new(fid1, 10), CallSiteId::new(fid3, 12), id3]
        );
    }

    #[test]
    fn callstackinterner_notices_duplicates() {
        let func1 = FunctionLocation::from_strings("a", "af");
        let func3 = FunctionLocation::from_strings("b", "bf");
        let fid1 = FunctionId::new(&func1 as *const FunctionLocation);
        let fid3 = FunctionId::new(&func3 as *const FunctionLocation);

        let mut cs1 = Callstack::new();
        cs1.start_call(0, CallSiteId::new(fid1, 2));
        let cs1b = cs1.clone();
        let mut cs2 = Callstack::new();
        cs2.start_call(0, CallSiteId::new(fid3, 4));
        let cs3 = Callstack::new();
        let cs3b = Callstack::new();

        let mut interner = CallstackInterner::new();

        let mut new = false;
        let id1 = interner.get_or_insert_id(&cs1, || new = true);
        assert!(new);

        new = false;
        let id1b = interner.get_or_insert_id(&cs1b, || new = true);
        assert!(!new);

        new = false;
        let id2 = interner.get_or_insert_id(&cs2, || new = true);
        assert!(new);

        new = false;
        let id3 = interner.get_or_insert_id(&cs3, || new = true);
        assert!(new);

        new = false;
        let id3b = interner.get_or_insert_id(&cs3b, || new = true);
        assert!(!new);

        assert_eq!(id1, id1b);
        assert_ne!(id1, id2);
        assert_ne!(id1, id3);
        assert_ne!(id2, id3);
        assert_eq!(id3, id3b);
        let mut expected = HashMap::default();
        expected.insert(id1, &cs1);
        expected.insert(id2, &cs2);
        expected.insert(id3, &cs3);
        assert_eq!(interner.get_reverse_map(), expected);
    }

    #[test]
    fn callstack_id_for_new_allocation() {
        let mut interner = CallstackInterner::new();

        let mut cs1 = Callstack::new();
        let id0 = cs1.id_for_new_allocation(0, |cs| interner.get_or_insert_id(cs, || ()));
        let id0b = cs1.id_for_new_allocation(0, |cs| interner.get_or_insert_id(cs, || ()));
        assert_eq!(id0, id0b);

        let func1 = FunctionLocation::from_strings("a", "af");
        let fid1 = FunctionId::new(&func1 as *const FunctionLocation);

        cs1.start_call(0, CallSiteId::new(fid1, 2));
        let id1 = cs1.id_for_new_allocation(1, |cs| interner.get_or_insert_id(cs, || ()));
        let id2 = cs1.id_for_new_allocation(2, |cs| interner.get_or_insert_id(cs, || ()));
        let id1b = cs1.id_for_new_allocation(1, |cs| interner.get_or_insert_id(cs, || ()));
        assert_eq!(id1, id1b);
        assert_ne!(id2, id0);
        assert_ne!(id2, id1);

        cs1.start_call(3, CallSiteId::new(fid1, 2));
        let id3 = cs1.id_for_new_allocation(4, |cs| interner.get_or_insert_id(cs, || ()));
        assert_ne!(id3, id0);
        assert_ne!(id3, id1);
        assert_ne!(id3, id2);

        cs1.finish_call();
        let id2b = cs1.id_for_new_allocation(2, |cs| interner.get_or_insert_id(cs, || ()));
        assert_eq!(id2, id2b);
        let id1c = cs1.id_for_new_allocation(1, |cs| interner.get_or_insert_id(cs, || ()));
        assert_eq!(id1, id1c);

        // Check for cache invalidation in start_call:
        cs1.start_call(1, CallSiteId::new(fid1, 1));
        let id4 = cs1.id_for_new_allocation(1, |cs| interner.get_or_insert_id(cs, || ()));
        assert_ne!(id4, id0);
        assert_ne!(id4, id1);
        assert_ne!(id4, id2);
        assert_ne!(id4, id3);

        // Check for cache invalidation in finish_call:
        cs1.finish_call();
        let id1d = cs1.id_for_new_allocation(1, |cs| interner.get_or_insert_id(cs, || ()));
        assert_eq!(id1, id1d);
    }

    #[test]
    fn peak_allocations_only_updated_on_new_peaks() {
        let func1 = FunctionLocation::from_strings("a", "af");
        let func3 = FunctionLocation::from_strings("b", "bf");
        let fid1 = FunctionId::new(&func1 as *const FunctionLocation);
        let fid3 = FunctionId::new(&func3 as *const FunctionLocation);

        let mut tracker = AllocationTracker::new(".".to_string());
        let mut cs1 = Callstack::new();
        cs1.start_call(0, CallSiteId::new(fid1, 2));
        let mut cs2 = Callstack::new();
        cs2.start_call(0, CallSiteId::new(fid3, 4));

        let cs1_id = tracker.get_callstack_id(&cs1);

        tracker.add_allocation(1, 1000, cs1_id);
        tracker.check_if_new_peak();
        // Peak should now match current allocations:
        assert_eq!(tracker.current_memory_usage, im::vector![1000]);
        assert_eq!(tracker.current_memory_usage, tracker.peak_memory_usage);
        assert_eq!(tracker.peak_allocated_bytes, 1000);
        let previous_peak = tracker.peak_memory_usage.clone();

        // Free the allocation:
        tracker.free_allocation(1);
        assert_eq!(tracker.current_allocated_bytes, 0);
        assert_eq!(tracker.current_memory_usage, im::vector![0]);
        assert_eq!(previous_peak, tracker.peak_memory_usage);
        assert_eq!(tracker.peak_allocated_bytes, 1000);

        // Add allocation, still less than 1000:
        tracker.add_allocation(3, 123, cs1_id);
        assert_eq!(tracker.current_memory_usage, im::vector![123]);
        tracker.check_if_new_peak();
        assert_eq!(previous_peak, tracker.peak_memory_usage);
        assert_eq!(tracker.peak_allocated_bytes, 1000);

        // Add allocation that goes past previous peak
        let cs2_id = tracker.get_callstack_id(&cs2);
        tracker.add_allocation(2, 2000, cs2_id);
        tracker.check_if_new_peak();
        assert_eq!(tracker.current_memory_usage, im::vector![123, 2000]);
        assert_eq!(tracker.current_memory_usage, tracker.peak_memory_usage);
        assert_eq!(tracker.peak_allocated_bytes, 2123);
        let previous_peak = tracker.peak_memory_usage.clone();

        // Add anonymous mmap() that doesn't go past previous peak:
        tracker.free_allocation(2);
        assert_eq!(tracker.current_memory_usage, im::vector![123, 0]);
        tracker.add_anon_mmap(50000, 1000, cs2_id);
        assert_eq!(tracker.current_memory_usage, im::vector![123, 1000]);
        tracker.check_if_new_peak();
        assert_eq!(tracker.current_allocated_bytes, 1123);
        assert_eq!(tracker.peak_allocated_bytes, 2123);
        assert_eq!(tracker.peak_memory_usage, previous_peak);
        assert_eq!(tracker.current_allocations.len(), 1);
        assert!(tracker.current_allocations.contains_key(&3));
        assert!(tracker.current_anon_mmaps.size() > 0);

        // Add anonymous mmap() that does go past previous peak:
        tracker.add_anon_mmap(600000, 2000, cs2_id);
        assert_eq!(tracker.current_memory_usage, im::vector![123, 3000]);
        tracker.check_if_new_peak();
        assert_eq!(tracker.current_memory_usage, tracker.peak_memory_usage);
        assert_eq!(tracker.current_allocated_bytes, 3123);
        assert_eq!(tracker.peak_allocated_bytes, 3123);

        // Remove mmap():
        tracker.free_anon_mmap(50000, 1000);
        assert_eq!(tracker.current_memory_usage, im::vector![123, 2000]);
        tracker.check_if_new_peak();
        assert_eq!(tracker.current_allocated_bytes, 2123);
        assert_eq!(tracker.peak_allocated_bytes, 3123);
        assert_eq!(tracker.current_anon_mmaps.size(), 2000);
        assert!(tracker
            .current_anon_mmaps
            .as_hashmap()
            .contains_key(&600000));

        // Partial removal of anonmyous mmap():
        tracker.free_anon_mmap(600100, 1000);
        assert_eq!(tracker.current_memory_usage, im::vector![123, 1000]);
        assert_eq!(tracker.current_allocated_bytes, 1123);
        assert_eq!(tracker.peak_allocated_bytes, 3123);
        assert_eq!(tracker.current_anon_mmaps.size(), 1000);
    }

    #[test]
    fn combine_callstacks_and_sum_allocations() {
        let func1 = FunctionLocation::from_strings("a", "af");
        let func2 = FunctionLocation::from_strings("b", "bf");
        let func3 = FunctionLocation::from_strings("c", "cf");

        let fid1 = FunctionId::new(&func1 as *const FunctionLocation);
        let fid2 = FunctionId::new(&func2 as *const FunctionLocation);
        let fid3 = FunctionId::new(&func3 as *const FunctionLocation);

        let mut tracker = AllocationTracker::new(".".to_string());
        let id1 = CallSiteId::new(fid1, 1);
        // Same function, different line number—should be different item:
        let id1_different = CallSiteId::new(fid1, 7);
        let id2 = CallSiteId::new(fid2, 2);

        let id3 = CallSiteId::new(fid3, 3);
        let mut cs1 = Callstack::new();
        cs1.start_call(0, id1);
        cs1.start_call(0, id2.clone());
        let mut cs2 = Callstack::new();
        cs2.start_call(0, id3);
        let mut cs3 = Callstack::new();
        cs3.start_call(0, id1_different);
        cs3.start_call(0, id2);
        let cs1_id = tracker.get_callstack_id(&cs1);
        let cs2_id = tracker.get_callstack_id(&cs2);
        let cs3_id = tracker.get_callstack_id(&cs3);
        tracker.add_allocation(1, 1000, cs1_id);
        tracker.add_allocation(2, 234, cs2_id);
        tracker.add_anon_mmap(3, 50000, cs1_id);
        tracker.add_allocation(4, 6000, cs3_id);

        // 234 allocation is too small, below the 99% total allocations
        // threshold, but we always guarantee at least 100 allocations.
        let mut expected = vec![
            "a:1 (af);TB@@a:1@@TB;b:2 (bf);TB@@b:2@@TB 51000".to_string(),
            "c:3 (cf);TB@@c:3@@TB 234".to_string(),
            "a:7 (af);TB@@a:7@@TB;b:2 (bf);TB@@b:2@@TB 6000".to_string(),
        ];
        let mut result: Vec<String> = tracker.to_lines(true, true).collect();
        result.sort();
        expected.sort();
        assert_eq!(expected, result);

        let mut expected2 = vec![
            "a:1 (af);b:2 (bf) 51000",
            "c:3 (cf) 234",
            "a:7 (af);b:2 (bf) 6000",
        ];
        let mut result2: Vec<String> = tracker.to_lines(true, false).collect();
        result2.sort();
        expected2.sort();
        assert_eq!(expected2, result2);
    }

    // TODO test to_lines(false)
}
