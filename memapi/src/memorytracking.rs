use crate::flamegraph::filter_to_useful_callstacks;
use crate::flamegraph::write_flamegraphs;
use crate::linecache::LineCacher;
use crate::python::get_runpy_path;

use super::rangemap::RangeMap;
use super::util::new_hashmap;
use ahash::RandomState as ARandomState;
use im::Vector as ImVector;
use itertools::Itertools;
use serde::Deserialize;
use serde::Serialize;
use std::borrow::Cow;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::path::Path;

extern "C" {
    fn _exit(exit_code: std::os::raw::c_int);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FunctionId(u64);

impl FunctionId {
    pub const UNKNOWN: Self = Self(u64::MAX);

    pub fn new(id: u64) -> Self {
        FunctionId(id)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

/// A function location in the Python source code, e.g. "example() in foo.py".
#[derive(Clone)]
struct FunctionLocation {
    filename: String,
    function_name: String,
}

pub trait FunctionLocations {
    fn get_function_and_filename(&self, id: FunctionId) -> (&str, &str);
}

/// Stores FunctionLocations, returns a FunctionId
#[derive(Clone)]
pub struct VecFunctionLocations {
    functions: Vec<FunctionLocation>,
}

impl VecFunctionLocations {
    /// Create a new tracker.
    pub fn new() -> Self {
        Self {
            functions: Vec::with_capacity(8192),
        }
    }

    /// Register a function, get back its id.
    pub fn add_function(&mut self, filename: String, function_name: String) -> FunctionId {
        self.functions.push(FunctionLocation {
            filename,
            function_name,
        });
        // If we ever have 2 ** 32 or more functions in our program, this will
        // break. Seems unlikely, even with long running workers.
        FunctionId((self.functions.len() - 1) as u64)
    }
}

impl FunctionLocations for VecFunctionLocations {
    /// Get the function name and filename.
    fn get_function_and_filename(&self, id: FunctionId) -> (&str, &str) {
        if id == FunctionId::UNKNOWN {
            return ("UNKNOWN", "UNKNOWN DUE TO BUG");
        }
        let location = &self.functions[id.0 as usize];
        (&location.function_name, &location.filename)
    }
}

/// Either the line number, or the bytecode index needed to get it.
#[derive(Copy, Clone, Serialize, Deserialize, Hash, Eq, PartialEq, Debug)]
pub enum LineNumberInfo {
    LineNumber(u32),
    BytecodeIndex(i32),
}

impl LineNumberInfo {
    fn get_line_number(&self) -> u32 {
        if let LineNumberInfo::LineNumber(lineno) = self {
            *lineno
        } else {
            debug_assert!(false, "This should never happen.");
            0
        }
    }
}

/// A specific location: file + function + line number.
#[derive(Clone, Debug, PartialEq, Eq, Copy, Hash, Serialize, Deserialize)]
pub struct CallSiteId {
    /// The function + filename. We use IDs for performance reasons (faster hashing).
    pub function: FunctionId,
    /// Line number within the _file_, 1-indexed.
    pub line_number: LineNumberInfo,
}

impl CallSiteId {
    pub fn new(function: FunctionId, line_number: LineNumberInfo) -> CallSiteId {
        CallSiteId {
            function,
            line_number,
        }
    }
}

/// The current Python callstack.
#[derive(Derivative, Serialize, Deserialize)]
#[derivative(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Callstack {
    calls: Vec<CallSiteId>,
    #[derivative(Hash = "ignore", PartialEq = "ignore")]
    cached_callstack_id: Option<(u32, CallstackId)>, // first bit is line number
}

impl Callstack {
    pub fn new() -> Callstack {
        Callstack {
            calls: Vec::new(),
            cached_callstack_id: None,
        }
    }

    pub fn from_vec(vec: Vec<CallSiteId>) -> Self {
        Self {
            calls: vec,
            cached_callstack_id: None,
        }
    }

    pub fn to_vec(&self) -> Vec<CallSiteId> {
        self.calls.clone()
    }

    pub fn start_call(&mut self, parent_line_number: u32, callsite_id: CallSiteId) {
        if parent_line_number != 0 {
            if let Some(mut call) = self.calls.last_mut() {
                call.line_number = LineNumberInfo::LineNumber(parent_line_number);
            }
        }
        self.calls.push(callsite_id);
        self.cached_callstack_id = None;
    }

    pub fn finish_call(&mut self) {
        self.calls.pop();
        self.cached_callstack_id = None;
    }

    pub fn id_for_new_allocation<F>(&mut self, line_number: u32, get_callstack_id: F) -> CallstackId
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
                call.line_number = LineNumberInfo::LineNumber(line_number);
            }
        }

        // Calculate callstack ID, cache it, and then return it;
        let callstack_id = get_callstack_id(self);
        self.cached_callstack_id = Some((line_number, callstack_id));
        callstack_id
    }

    pub fn as_string(
        &self,
        to_be_post_processed: bool,
        functions: &dyn FunctionLocations,
        separator: &'static str,
        linecache: &mut LineCacher,
    ) -> String {
        if self.calls.is_empty() {
            return "[No Python stack]".to_string();
        }
        let calls: Vec<(CallSiteId, (&str, &str))> = self
            .calls
            .iter()
            .map(|id| (*id, functions.get_function_and_filename(id.function)))
            .collect();
        let skip_prefix = if cfg!(feature = "fil4prod") {
            0
        } else {
            // Due to implementation details we have some runpy() frames at the
            // start; remove them.
            runpy_prefix_length(calls.iter())
        };
        calls
            .into_iter()
            .skip(skip_prefix)
            .map(|(id, (function, filename))| {
                if to_be_post_processed {
                    // Get Python code.
                    let code = linecache
                        .get_source_line(filename, id.line_number.get_line_number() as usize);
                    // Leading whitespace is dropped by SVG, so we'd like to
                    // replace it with non-breaking space. However, inferno
                    // trims whitespace
                    // (https://github.com/jonhoo/inferno/blob/de3f7d94d4718bfee57655c1fddd4d2714bc78d0/src/flamegraph/merge.rs#L126)
                    // and that causes incorrect "unsorted lines" errors
                    // which I can't be bothered to fix right now, so for
                    // now do hack where we shove in some other character
                    // that can be fixed in post-processing.
                    let code = code.replace(" ", "\u{12e4}");
                    // Semicolons are used as separator in the flamegraph
                    // input format, so need to replace them with some other
                    // character. We use "full-width semicolon", and then
                    // replace it back in post-processing.
                    let code = code.replace(";", "\u{ff1b}");
                    // The \u{2800} is to ensure we don't have empty lines,
                    // and that whitespace doesn't get trimmed from start;
                    // we'll get rid of this in post-processing.
                    format!(
                        "{filename}:{line} ({function});\u{2800}{code}",
                        filename = filename,
                        line = id.line_number.get_line_number(),
                        function = function,
                        code = &code.trim_end(),
                    )
                } else {
                    format!(
                        "{filename}:{line} ({function})",
                        filename = filename,
                        line = id.line_number.get_line_number(),
                        function = function,
                    )
                }
            })
            .join(separator)
    }
}

fn runpy_prefix_length(calls: std::slice::Iter<(CallSiteId, (&str, &str))>) -> usize {
    let mut length = 0;
    let runpy_path = get_runpy_path();
    for (_, (_, filename)) in calls {
        // On Python 3.11 it uses <frozen runpy> for some reason.
        if *filename == runpy_path || *filename == "<frozen runpy>" {
            length += 1;
        } else {
            return length;
        }
    }
    0
}

pub type CallstackId = u32;

/// Maps Functions to integer identifiers used in CallStacks.
pub struct CallstackInterner {
    max_id: CallstackId,
    callstack_to_id: HashMap<Callstack, u32, ARandomState>,
}

impl CallstackInterner {
    pub fn new() -> Self {
        CallstackInterner {
            max_id: 0,
            callstack_to_id: new_hashmap(),
        }
    }

    /// Add a (possibly) new Function, returning its ID.
    pub fn get_or_insert_id<F: FnOnce()>(
        &mut self,
        callstack: Cow<Callstack>,
        call_on_new: F,
    ) -> CallstackId {
        let max_id = &mut self.max_id;
        if let Some(result) = self.callstack_to_id.get(&*callstack) {
            *result
        } else {
            let new_id = *max_id;
            *max_id += 1;
            self.callstack_to_id.insert(callstack.into_owned(), new_id);
            call_on_new();
            new_id
        }
    }

    /// Get map from IDs to Callstacks.
    fn get_reverse_map(&self) -> HashMap<CallstackId, &Callstack, ARandomState> {
        let mut result = new_hashmap();
        for (call_site, csid) in self.callstack_to_id.iter() {
            result.insert(*csid, call_site);
        }
        result
    }
}

const MIB: usize = 1024 * 1024;
const HIGH_32BIT: u32 = 1 << 31;

/// A unique identifier for a process.
#[derive(Clone, Copy, Debug, PartialEq, Ord, PartialOrd, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProcessUid(pub u32);

pub const PARENT_PROCESS: ProcessUid = ProcessUid(0);

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
    fn new(callstack_id: CallstackId, size: usize) -> Self {
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

    fn size(&self) -> usize {
        if self.compressed_size >= HIGH_32BIT {
            (self.compressed_size - HIGH_32BIT) as usize * MIB
        } else {
            self.compressed_size as usize
        }
    }
}

fn same_callstack(callstack: &Callstack) -> Cow<Callstack> {
    Cow::Borrowed(callstack)
}

/// The main data structure tracking everything.
pub struct AllocationTracker<FL: FunctionLocations> {
    // malloc()/calloc():
    current_allocations: BTreeMap<ProcessUid, HashMap<usize, Allocation, ARandomState>>,
    // anonymous mmap(), i.e. not file backed:
    current_anon_mmaps: BTreeMap<ProcessUid, RangeMap<CallstackId>>,

    // Map FunctionIds to function + filename strings, so we can store the
    // former and save memory.
    pub functions: FL,

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

    // Allocations that somehow disappeared. Not relevant for sampling profiler.
    missing_allocated_bytes: usize,

    // free()/realloc() of unknown address. Not relevant for sampling profiler.
    failed_deallocations: usize,
}

impl<FL: FunctionLocations> AllocationTracker<FL> {
    pub fn new(default_path: String, functions: FL) -> AllocationTracker<FL> {
        AllocationTracker {
            current_allocations: BTreeMap::from([(PARENT_PROCESS, new_hashmap())]),
            current_anon_mmaps: BTreeMap::from([(PARENT_PROCESS, RangeMap::new())]),
            interner: CallstackInterner::new(),
            current_memory_usage: ImVector::new(),
            peak_memory_usage: ImVector::new(),
            functions,
            current_allocated_bytes: 0,
            peak_allocated_bytes: 0,
            missing_allocated_bytes: 0,
            failed_deallocations: 0,
            default_path,
        }
    }

    /// Print a traceback for the given CallstackId.
    pub fn print_traceback(&self, message: &'static str, callstack_id: CallstackId) {
        let id_to_callstack = self.interner.get_reverse_map();
        let callstack = id_to_callstack[&callstack_id];
        eprintln!("=fil-profile= {}", message);
        eprintln!(
            "=| {}",
            callstack.as_string(false, &self.functions, "\n=| ", &mut LineCacher::default())
        );
    }

    pub fn get_current_allocated_bytes(&self) -> usize {
        self.current_allocated_bytes
    }

    pub fn get_peak_allocated_bytes(&self) -> usize {
        self.peak_allocated_bytes
    }

    pub fn get_allocation_size(&self, process: ProcessUid, address: usize) -> usize {
        if let Some(allocation) = self
            .current_allocations
            .get(&process)
            .map(|a| a.get(&address))
            .flatten()
        {
            allocation.size()
        } else {
            0
        }
    }

    /// Check if a new peak has been reached:
    pub fn check_if_new_peak(&mut self) {
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

    pub fn get_callstack_id(&mut self, callstack: &Callstack) -> CallstackId {
        let current_memory_usage = &mut self.current_memory_usage;
        self.interner
            .get_or_insert_id(Cow::Borrowed(callstack), || {
                current_memory_usage.push_back(0)
            })
    }

    /// Add a new allocation based off the current callstack.
    pub fn add_allocation(
        &mut self,
        process: ProcessUid,
        address: usize,
        size: usize,
        callstack_id: CallstackId,
    ) {
        let alloc = Allocation::new(callstack_id, size);
        let compressed_size = alloc.size();
        if let Some(previous) = self
            .current_allocations
            .entry(process)
            .or_default()
            .insert(address, alloc)
        {
            // In production use (proposed commercial product) allocations are
            // only sampled, so missing allocations are common and not the sign
            // of an error.
            #[cfg(not(feature = "fil4prod"))]
            {
                // I've seen this happen on macOS only in some threaded code
                // (malloc_on_thread_exit test). Not sure why, but difference was
                // only 16 bytes, which shouldn't have real impact on profiling
                // outcomes. Apparently also happening on Linux, hope to fix this
                // soon (https://github.com/pythonspeed/filprofiler/issues/149).
                self.missing_allocated_bytes += previous.size();
                // Cleanup the previous allocation, since we never saw its free():
                self.remove_memory_usage(previous.callstack_id, previous.size());
                if *crate::util::DEBUG_MODE {
                    self.print_traceback(
                        "The allocation from this traceback disappeared:",
                        previous.callstack_id,
                    );
                    self.print_traceback(
                        "The current traceback that overwrote the disappearing allocation:",
                        alloc.callstack_id,
                    );
                    eprintln!(
                        "|= The current C/Rust backtrace: {:?}",
                        backtrace::Backtrace::new()
                    );
                }
            }
        }
        self.add_memory_usage(callstack_id, compressed_size as usize);
    }

    /// Free an existing allocation, return how much was removed, if any.
    pub fn free_allocation(&mut self, process: ProcessUid, address: usize) -> Option<usize> {
        // Before we reduce memory, let's check if we've previously hit a peak:
        self.check_if_new_peak();

        if let Some(removed) = self
            .current_allocations
            .entry(process)
            .or_default()
            .remove(&address)
        {
            self.remove_memory_usage(removed.callstack_id, removed.size());
            Some(removed.size())
        } else {
            // This allocation doesn't exist; often this will be something
            // allocated before Fil tracking was started, but it might also be a
            // bug.
            #[cfg(not(feature = "fil4prod"))]
            if *crate::util::DEBUG_MODE {
                self.failed_deallocations += 1;
                eprintln!(
                    "=fil-profile= Your program attempted to free an allocation at an address we don't know about:"
                );
                eprintln!("=| {:?}", backtrace::Backtrace::new());
            }
            None
        }
    }

    /// Add a new anonymous mmap() based of the current callstack.
    pub fn add_anon_mmap(
        &mut self,
        process: ProcessUid,
        address: usize,
        size: usize,
        callstack_id: CallstackId,
    ) {
        self.current_anon_mmaps
            .entry(process)
            .or_default()
            .add(address, size, callstack_id);
        self.add_memory_usage(callstack_id, size);
    }

    pub fn free_anon_mmap(&mut self, process: ProcessUid, address: usize, size: usize) {
        // Before we reduce memory, let's check if we've previously hit a peak:
        self.check_if_new_peak();
        // Now remove, and update totoal memory tracking:
        for (callstack_id, removed) in self
            .current_anon_mmaps
            .entry(process)
            .or_default()
            .remove(address, size)
        {
            self.remove_memory_usage(callstack_id, removed);
        }
    }

    /// The process just died, remove all the allocations.
    pub fn drop_process(&mut self, process: ProcessUid) {
        // Before we reduce memory, let's check if we've previously hit a peak:
        self.check_if_new_peak();

        // Drop anon mmaps, call remove_memory_usage on all entries.
        if let Some(mmaps_for_process) = self.current_anon_mmaps.remove(&process) {
            for (size, callstack_id) in mmaps_for_process.into_iter() {
                self.remove_memory_usage(callstack_id, size);
            }
        }

        // Drop allocations, call remove_memory_usage on all entries.
        if let Some(allocations_for_process) = self.current_allocations.remove(&process) {
            for allocation in allocations_for_process.values() {
                self.remove_memory_usage(allocation.callstack_id, allocation.size());
            }
        }
    }

    /// Combine Callstacks and make them human-readable. Duplicate callstacks
    /// have their allocated memory summed.
    fn combine_callstacks(
        &self,
        // If false, will do the current allocations:
        peak: bool,
    ) -> HashMap<CallstackId, usize, ARandomState> {
        // Would be nice to validate if data is consistent. However, there are
        // edge cases that make it slightly inconsistent (e.g. see the
        // unexpected code path in add_allocation() above), and blowing up
        // without giving the user their data just because of a small
        // inconsistency doesn't seem ideal. Perhaps if validate() merely
        // reported problems, or maybe validate() should only be enabled in
        // development mode.
        //self.validate();

        // We get a LOT of tiny allocations. To reduce overhead of creating
        // flamegraph (which currently loads EVERYTHING into memory), just do
        // the top 99% of allocations.
        let callstacks = if peak {
            &self.peak_memory_usage
        } else {
            &self.current_memory_usage
        };
        let sum = callstacks.iter().sum();
        filter_to_useful_callstacks(callstacks.iter().enumerate(), sum)
            .into_iter()
            .map(|(k, v)| (k as CallstackId, v))
            .collect()
    }

    /// Dump all callstacks in peak memory usage to various files describing the
    /// memory usage.
    pub fn dump_peak_to_flamegraph(&mut self, path: &str) {
        self.dump_to_flamegraph(path, true, "peak-memory", "Peak Tracked Memory Usage", true);
    }

    pub fn to_lines<'a, F>(
        &'a self,
        peak: bool,
        to_be_post_processed: bool,
        update_callstack: F,
    ) -> impl ExactSizeIterator<Item = String> + '_
    where
        F: Fn(&Callstack) -> Cow<Callstack> + 'a,
    {
        let by_call = self.combine_callstacks(peak).into_iter();
        let id_to_callstack = self.interner.get_reverse_map();
        let mut linecache = LineCacher::default();
        by_call.map(move |(callstack_id, size)| {
            format!(
                "{} {}",
                update_callstack(id_to_callstack.get(&callstack_id).unwrap()).as_string(
                    to_be_post_processed,
                    &self.functions,
                    ";",
                    &mut linecache,
                ),
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
        // First, make sure peaks are correct:
        self.check_if_new_peak();

        // Print warning if we're missing allocations.
        #[cfg(not(feature = "fil4prod"))]
        {
            let allocated_bytes = if peak {
                self.peak_allocated_bytes
            } else {
                self.current_allocated_bytes
            };
            if self.missing_allocated_bytes > 0 {
                eprintln!("=fil-profile= WARNING: {:.2}% ({} bytes) of tracked memory somehow disappeared. If this is a small percentage you can just ignore this warning, since the missing allocations won't impact the profiling results. If the % is high, please run `export FIL_DEBUG=1` to get more output', re-run Fil on your script, and then file a bug report at https://github.com/pythonspeed/filprofiler/issues/new", self.missing_allocated_bytes as f64 * 100.0 / allocated_bytes as f64, self.missing_allocated_bytes);
            }
            if self.failed_deallocations > 0 {
                eprintln!("=fil-profile= WARNING: Encountered {} deallocations of untracked allocations. A certain number are expected in normal operation, of allocations created before Fil started tracking, and even more if you're using the Fil API to turn tracking on and off.", self.failed_deallocations);
            }
        }

        eprintln!("=fil-profile= Preparing to write to {}", path);
        let directory_path = Path::new(path);

        let title = format!(
            "{} ({:.1} MiB)",
            title,
            self.peak_allocated_bytes as f64 / (1024.0 * 1024.0)
        );
        #[cfg(not(feature = "fil4prod"))]
        let subtitle = r#"Made with the Fil profiler. <a href="https://pythonspeed.com/fil/" style="text-decoration: underline;" target="_parent">Try it on your code!</a>"#;
        #[cfg(feature = "fil4prod")]
        let subtitle = r#"Made with the Fil4prod profiler. <a href="https://pythonspeed.com/products/fil4prod/" style="text-decoration: underline;" target="_parent">Try it on your code!</a>"#;
        write_flamegraphs(
            directory_path,
            base_filename,
            &title,
            subtitle,
            "bytes",
            to_be_post_processed,
            |tbpp| self.to_lines(peak, tbpp, same_callstack),
        )
    }

    /// Clear memory we won't be needing anymore, since we're going to exit out.
    pub fn oom_break_glass(&mut self) {
        self.current_allocations.clear();
        self.peak_memory_usage.clear();
    }

    /// Dump information about where we are.
    pub fn oom_dump(&mut self) {
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
            _exit(53);
        }
    }

    /// Validate internal state is in a good state. This won't pass until
    /// check_if_new_peak() is called.
    fn validate(&self) {
        assert!(self.peak_allocated_bytes >= self.current_allocated_bytes);
        let current_allocations: usize = self
            .current_anon_mmaps
            .values()
            .map(|maps| maps.size())
            .sum::<usize>()
            + self
                .current_allocations
                .values()
                .flat_map(|allocs| allocs.iter())
                .map(|(_, alloc)| alloc.size())
                .sum::<usize>();
        assert!(
            current_allocations == self.current_allocated_bytes,
            "{} != {}",
            current_allocations,
            self.current_allocated_bytes
        );
        assert!(self.current_memory_usage.iter().sum::<usize>() == self.current_allocated_bytes);
        assert!(self.peak_memory_usage.iter().sum::<usize>() == self.peak_allocated_bytes);
    }

    /// Reset internal state in way that doesn't invalidate e.g. thread-local
    /// caching of callstack ID.
    pub fn reset(&mut self, default_path: String) {
        self.current_allocations.clear();
        self.current_anon_mmaps = BTreeMap::from([(PARENT_PROCESS, RangeMap::new())]);
        for i in self.current_memory_usage.iter_mut() {
            *i = 0;
        }
        self.peak_memory_usage = ImVector::new();
        self.current_allocated_bytes = 0;
        self.peak_allocated_bytes = 0;
        self.default_path = default_path;
        self.validate();
    }
}

#[cfg(test)]
mod tests {
    use crate::memorytracking::{same_callstack, ProcessUid, PARENT_PROCESS};

    use super::LineNumberInfo::LineNumber;
    use super::{
        Allocation, AllocationTracker, CallSiteId, Callstack, CallstackInterner, FunctionId,
        FunctionLocations, VecFunctionLocations, HIGH_32BIT, MIB,
    };
    use proptest::prelude::*;
    use std::borrow::Cow;
    use std::collections::HashMap;

    fn new_tracker() -> AllocationTracker<VecFunctionLocations> {
        AllocationTracker::new(".".to_string(), VecFunctionLocations::new())
    }

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
            let mut tracker = new_tracker();
            let cs_id = tracker.get_callstack_id(&Callstack::new());
            tracker.add_allocation(PARENT_PROCESS, 0, size, cs_id);
            tracker.add_anon_mmap(PARENT_PROCESS, 1, size * 2, cs_id);
            // We don't track (large) allocations exactly right, but they should
            // be quite close:
            let ratio = ((size * 3) as f64) / (tracker.current_memory_usage[0] as f64);
            prop_assert!(0.999 < ratio);
            prop_assert!(ratio < 1.001);
            tracker.free_allocation(PARENT_PROCESS, 0);
            tracker.free_anon_mmap(PARENT_PROCESS, 1, size * 2);
            // Once we've freed everything, it should be _exactly_ 0.
            prop_assert_eq!(&im::vector![0], &tracker.current_memory_usage);
            tracker.check_if_new_peak();
            tracker.validate();
        }

        #[test]
        fn current_allocated_matches_sum_of_allocations(
            // Allocated bytes. Will use index as the memory address.
            allocated_sizes in prop::collection::vec((0..2 as u32, 1..100 as usize), 10..20),
            // Allocations to free.
            free_indices in prop::collection::btree_set(0..10 as usize, 1..5)
        ) {
            let mut tracker = new_tracker();
            let mut expected_memory_usage = im::vector![];
            for i in 0..allocated_sizes.len() {
                let (process, allocation_size) = *allocated_sizes.get(i).unwrap();
                let process = ProcessUid(process);
                let mut cs = Callstack::new();
                cs.start_call(0, CallSiteId::new(FunctionId::new(i as u64), LineNumber(0)));
                let cs_id = tracker.get_callstack_id(&cs);
                tracker.add_allocation(process, i as usize, allocation_size, cs_id);
                expected_memory_usage.push_back(allocation_size);
            }
            let mut expected_sum = allocated_sizes.iter().map(|t| t.1).sum();
            let expected_peak : usize = expected_sum;
            prop_assert_eq!(tracker.current_allocated_bytes, expected_sum);
            prop_assert_eq!(&tracker.current_memory_usage, &expected_memory_usage);
            for i in free_indices.iter() {
                let (process, expected_removed) = allocated_sizes.get(*i).unwrap();
                let process = ProcessUid(*process);
                expected_sum -= expected_removed;
                let removed = tracker.free_allocation(process, *i);
                prop_assert_eq!(removed, Some(*expected_removed));
                expected_memory_usage[*i] -= expected_removed;
                prop_assert_eq!(tracker.current_allocated_bytes, expected_sum);
                prop_assert_eq!(&tracker.current_memory_usage, &expected_memory_usage);
            }
            prop_assert_eq!(tracker.peak_allocated_bytes, expected_peak);
            tracker.check_if_new_peak();
            tracker.validate();
        }

        #[test]
        fn current_allocated_anon_maps_matches_sum_of_allocations(
            // Allocated bytes. Will use index as the memory address.
            allocated_sizes in prop::collection::vec((0..2 as u32, 1..100 as usize), 10..20),
            // Allocations to free.
            free_indices in prop::collection::btree_set(0..10 as usize, 1..5)
        ) {
            let mut tracker = new_tracker();
            let mut expected_memory_usage = im::vector![];
            // Make sure addresses don't overlap:
            let addresses : Vec<usize> = (0..allocated_sizes.len()).map(|i| i * 10000).collect();
            for i in 0..allocated_sizes.len() {
                let (process, allocation_size) = *allocated_sizes.get(i).unwrap();
                let process = ProcessUid(process);
                let mut cs = Callstack::new();
                cs.start_call(0, CallSiteId::new(FunctionId::new(i as u64),  LineNumber(0)));
                let csid = tracker.get_callstack_id(&cs);
                tracker.add_anon_mmap(process, addresses[i] as usize, allocation_size, csid);
                expected_memory_usage.push_back(allocation_size);
            }
            let mut expected_sum = allocated_sizes.iter().map(|t|t.1).sum();
            let expected_peak : usize = expected_sum;
            prop_assert_eq!(tracker.current_allocated_bytes, expected_sum);
            prop_assert_eq!(&tracker.current_memory_usage, &expected_memory_usage);
            for i in free_indices.iter() {
                let (process, allocation_size) = *allocated_sizes.get(*i).unwrap();
                let process = ProcessUid(process);
                expected_sum -= allocation_size;
                tracker.free_anon_mmap(process, addresses[*i], allocation_size);
                expected_memory_usage[*i] -= allocation_size;
                prop_assert_eq!(tracker.current_allocated_bytes, expected_sum);
                prop_assert_eq!(&tracker.current_memory_usage, &expected_memory_usage);
            }
            prop_assert_eq!(tracker.peak_allocated_bytes, expected_peak);
            tracker.check_if_new_peak();
            tracker.validate();
        }

        #[test]
        fn drop_process_removes_that_process_allocations_and_mmaps(
            // Allocated bytes. Will use index as the memory address.
            allocated_sizes in prop::collection::vec((0..2 as u32, 1..100 as usize), 10..20),
            allocated_mmaps in prop::collection::vec((0..2 as u32, 1..100 as usize), 10..20),
        ) {
            let mut tracker = new_tracker();
            let mut expected_memory_usage : usize = 0;
            // Make sure addresses don't overlap:
            let mmap_addresses : Vec<usize> = (0..allocated_mmaps.len()).map(|i| i * 10000).collect();
            for i in 0..allocated_sizes.len() {
                let (process, allocation_size) = *allocated_sizes.get(i).unwrap();
                let process = ProcessUid(process);
                let mut cs = Callstack::new();
                cs.start_call(0, CallSiteId::new(FunctionId::new(i as u64), LineNumber(0)));
                let cs_id = tracker.get_callstack_id(&cs);
                tracker.add_allocation(process, i as usize, allocation_size, cs_id);
                expected_memory_usage += allocation_size;
            }
            for i in 0..allocated_mmaps.len() {
                let (process, allocation_size) = *allocated_mmaps.get(i).unwrap();
                let process = ProcessUid(process);
                let mut cs = Callstack::new();
                cs.start_call(0, CallSiteId::new(FunctionId::new(i as u64), LineNumber(0)));
                let csid = tracker.get_callstack_id(&cs);
                tracker.add_anon_mmap(process, mmap_addresses[i] as usize, allocation_size, csid);
                expected_memory_usage += allocation_size;
            }
            prop_assert_eq!(tracker.current_allocated_bytes, expected_memory_usage);
            let expected_peak = expected_memory_usage;
            let to_drop = ProcessUid(1);
            tracker.drop_process(to_drop);
            expected_memory_usage -= allocated_sizes.iter().filter(|(p, _)| ProcessUid(*p) == to_drop).map(|(_, size)| size).sum::<usize>();
            expected_memory_usage -= allocated_mmaps.iter().filter(|(p, _)| ProcessUid(*p) == to_drop).map(|(_, size)| size).sum::<usize>();
            prop_assert_eq!(tracker.current_allocated_bytes, expected_memory_usage);
            prop_assert_eq!(tracker.peak_allocated_bytes, expected_peak);
            tracker.check_if_new_peak();
            tracker.validate();
        }

    }

    #[test]
    fn untracked_allocation_removal() {
        let mut tracker = new_tracker();
        assert_eq!(tracker.free_allocation(PARENT_PROCESS, 123), None);
    }

    #[test]
    fn callstack_line_numbers() {
        let fid1 = FunctionId::new(1u64);
        let fid3 = FunctionId::new(3u64);
        let fid5 = FunctionId::new(5u64);

        // Parent line number does nothing if it's first call:
        let mut cs1 = Callstack::new();
        // CallSiteId::new($a, $b) ==>> CallSiteId::new($a, LineNumber($b))
        let id1 = CallSiteId::new(fid1, LineNumber(2));
        let id2 = CallSiteId::new(fid3, LineNumber(45));
        let id3 = CallSiteId::new(fid5, LineNumber(6));
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
            vec![
                CallSiteId::new(fid1, LineNumber(10)),
                CallSiteId::new(fid3, LineNumber(12)),
                id3
            ]
        );
    }

    #[test]
    fn callstackinterner_notices_duplicates() {
        let fid1 = FunctionId::new(1u64);
        let fid3 = FunctionId::new(3u64);

        let mut cs1 = Callstack::new();
        cs1.start_call(0, CallSiteId::new(fid1, LineNumber(2)));
        let cs1b = cs1.clone();
        let mut cs2 = Callstack::new();
        cs2.start_call(0, CallSiteId::new(fid3, LineNumber(4)));
        let cs3 = Callstack::new();
        let cs3b = Callstack::new();

        let mut interner = CallstackInterner::new();

        let mut new = false;
        let id1 = interner.get_or_insert_id(Cow::Borrowed(&cs1), || new = true);
        assert!(new);

        new = false;
        let id1b = interner.get_or_insert_id(Cow::Borrowed(&cs1b), || new = true);
        assert!(!new);

        new = false;
        let id2 = interner.get_or_insert_id(Cow::Borrowed(&cs2), || new = true);
        assert!(new);

        new = false;
        let id3 = interner.get_or_insert_id(Cow::Borrowed(&cs3), || new = true);
        assert!(new);

        new = false;
        let id3b = interner.get_or_insert_id(Cow::Borrowed(&cs3b), || new = true);
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
        let id0 =
            cs1.id_for_new_allocation(0, |cs| interner.get_or_insert_id(Cow::Borrowed(&cs), || ()));
        let id0b =
            cs1.id_for_new_allocation(0, |cs| interner.get_or_insert_id(Cow::Borrowed(&cs), || ()));
        assert_eq!(id0, id0b);

        let fid1 = FunctionId::new(1u64);

        cs1.start_call(0, CallSiteId::new(fid1, LineNumber(2)));
        let id1 =
            cs1.id_for_new_allocation(1, |cs| interner.get_or_insert_id(Cow::Borrowed(&cs), || ()));
        let id2 =
            cs1.id_for_new_allocation(2, |cs| interner.get_or_insert_id(Cow::Borrowed(&cs), || ()));
        let id1b =
            cs1.id_for_new_allocation(1, |cs| interner.get_or_insert_id(Cow::Borrowed(&cs), || ()));
        assert_eq!(id1, id1b);
        assert_ne!(id2, id0);
        assert_ne!(id2, id1);

        cs1.start_call(3, CallSiteId::new(fid1, LineNumber(2)));
        let id3 =
            cs1.id_for_new_allocation(4, |cs| interner.get_or_insert_id(Cow::Borrowed(&cs), || ()));
        assert_ne!(id3, id0);
        assert_ne!(id3, id1);
        assert_ne!(id3, id2);

        cs1.finish_call();
        let id2b =
            cs1.id_for_new_allocation(2, |cs| interner.get_or_insert_id(Cow::Borrowed(&cs), || ()));
        assert_eq!(id2, id2b);
        let id1c =
            cs1.id_for_new_allocation(1, |cs| interner.get_or_insert_id(Cow::Borrowed(&cs), || ()));
        assert_eq!(id1, id1c);

        // Check for cache invalidation in start_call:
        cs1.start_call(1, CallSiteId::new(fid1, LineNumber(1)));
        let id4 =
            cs1.id_for_new_allocation(1, |cs| interner.get_or_insert_id(Cow::Borrowed(&cs), || ()));
        assert_ne!(id4, id0);
        assert_ne!(id4, id1);
        assert_ne!(id4, id2);
        assert_ne!(id4, id3);

        // Check for cache invalidation in finish_call:
        cs1.finish_call();
        let id1d =
            cs1.id_for_new_allocation(1, |cs| interner.get_or_insert_id(Cow::Borrowed(&cs), || ()));
        assert_eq!(id1, id1d);
    }

    #[test]
    fn peak_allocations_only_updated_on_new_peaks() {
        let fid1 = FunctionId::new(1u64);
        let fid3 = FunctionId::new(3u64);

        let mut tracker = new_tracker();
        let mut cs1 = Callstack::new();
        cs1.start_call(0, CallSiteId::new(fid1, LineNumber(2)));
        let mut cs2 = Callstack::new();
        cs2.start_call(0, CallSiteId::new(fid3, LineNumber(4)));

        let cs1_id = tracker.get_callstack_id(&cs1);

        tracker.add_allocation(PARENT_PROCESS, 1, 1000, cs1_id);
        tracker.check_if_new_peak();
        // Peak should now match current allocations:
        assert_eq!(tracker.current_memory_usage, im::vector![1000]);
        assert_eq!(tracker.current_memory_usage, tracker.peak_memory_usage);
        assert_eq!(tracker.peak_allocated_bytes, 1000);
        let previous_peak = tracker.peak_memory_usage.clone();

        // Free the allocation:
        tracker.free_allocation(PARENT_PROCESS, 1);
        assert_eq!(tracker.current_allocated_bytes, 0);
        assert_eq!(tracker.current_memory_usage, im::vector![0]);
        assert_eq!(previous_peak, tracker.peak_memory_usage);
        assert_eq!(tracker.peak_allocated_bytes, 1000);

        // Add allocation, still less than 1000:
        tracker.add_allocation(PARENT_PROCESS, 3, 123, cs1_id);
        assert_eq!(tracker.current_memory_usage, im::vector![123]);
        tracker.check_if_new_peak();
        assert_eq!(previous_peak, tracker.peak_memory_usage);
        assert_eq!(tracker.peak_allocated_bytes, 1000);

        // Add allocation that goes past previous peak
        let cs2_id = tracker.get_callstack_id(&cs2);
        tracker.add_allocation(PARENT_PROCESS, 2, 2000, cs2_id);
        tracker.check_if_new_peak();
        assert_eq!(tracker.current_memory_usage, im::vector![123, 2000]);
        assert_eq!(tracker.current_memory_usage, tracker.peak_memory_usage);
        assert_eq!(tracker.peak_allocated_bytes, 2123);
        let previous_peak = tracker.peak_memory_usage.clone();

        // Add anonymous mmap() that doesn't go past previous peak:
        tracker.free_allocation(PARENT_PROCESS, 2);
        assert_eq!(tracker.current_memory_usage, im::vector![123, 0]);
        tracker.add_anon_mmap(PARENT_PROCESS, 50000, 1000, cs2_id);
        assert_eq!(tracker.current_memory_usage, im::vector![123, 1000]);
        tracker.check_if_new_peak();
        assert_eq!(tracker.current_allocated_bytes, 1123);
        assert_eq!(tracker.peak_allocated_bytes, 2123);
        assert_eq!(tracker.peak_memory_usage, previous_peak);
        assert_eq!(tracker.current_allocations.len(), 1);
        assert!(tracker.current_allocations[&PARENT_PROCESS].contains_key(&3));
        assert!(tracker.current_anon_mmaps[&PARENT_PROCESS].size() > 0);

        // Add anonymous mmap() that does go past previous peak:
        tracker.add_anon_mmap(PARENT_PROCESS, 600000, 2000, cs2_id);
        assert_eq!(tracker.current_memory_usage, im::vector![123, 3000]);
        tracker.check_if_new_peak();
        assert_eq!(tracker.current_memory_usage, tracker.peak_memory_usage);
        assert_eq!(tracker.current_allocated_bytes, 3123);
        assert_eq!(tracker.peak_allocated_bytes, 3123);

        // Remove mmap():
        tracker.free_anon_mmap(PARENT_PROCESS, 50000, 1000);
        assert_eq!(tracker.current_memory_usage, im::vector![123, 2000]);
        tracker.check_if_new_peak();
        assert_eq!(tracker.current_allocated_bytes, 2123);
        assert_eq!(tracker.peak_allocated_bytes, 3123);
        assert_eq!(tracker.current_anon_mmaps[&PARENT_PROCESS].size(), 2000);
        assert!(tracker.current_anon_mmaps[&PARENT_PROCESS]
            .as_hashmap()
            .contains_key(&600000));

        // Partial removal of anonmyous mmap():
        tracker.free_anon_mmap(PARENT_PROCESS, 600100, 1000);
        assert_eq!(tracker.current_memory_usage, im::vector![123, 1000]);
        assert_eq!(tracker.current_allocated_bytes, 1123);
        assert_eq!(tracker.peak_allocated_bytes, 3123);
        assert_eq!(tracker.current_anon_mmaps[&PARENT_PROCESS].size(), 1000);
        tracker.check_if_new_peak();
        tracker.validate();
    }

    #[test]
    fn combine_callstacks_and_sum_allocations() {
        pyo3::prepare_freethreaded_python();
        let mut tracker = new_tracker();
        let fid1 = tracker
            .functions
            .add_function("a".to_string(), "af".to_string());
        let fid2 = tracker
            .functions
            .add_function("b".to_string(), "bf".to_string());
        let fid3 = tracker
            .functions
            .add_function("c".to_string(), "cf".to_string());

        let id1 = CallSiteId::new(fid1, LineNumber(1));
        // Same function, different line number—should be different item:
        let id1_different = CallSiteId::new(fid1, LineNumber(7));
        let id2 = CallSiteId::new(fid2, LineNumber(2));

        let id3 = CallSiteId::new(fid3, LineNumber(3));
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
        tracker.add_allocation(PARENT_PROCESS, 1, 1000, cs1_id);
        tracker.add_allocation(PARENT_PROCESS, 2, 234, cs2_id);
        tracker.add_anon_mmap(PARENT_PROCESS, 3, 50000, cs1_id);
        tracker.add_allocation(PARENT_PROCESS, 4, 6000, cs3_id);

        // Make sure we notice new peak.
        tracker.check_if_new_peak();

        // 234 allocation is too small, below the 99% total allocations
        // threshold, but we always guarantee at least 100 allocations.

        // TODO figure out how to test this...
        // let mut expected = vec![
        //     "a:1 (af);TB@@a:1@@TB;b:2 (bf);TB@@b:2@@TB 51000".to_string(),
        //     "c:3 (cf);TB@@c:3@@TB 234".to_string(),
        //     "a:7 (af);TB@@a:7@@TB;b:2 (bf);TB@@b:2@@TB 6000".to_string(),
        // ];
        // let mut result: Vec<String> = tracker.to_lines(true, true).collect();
        // result.sort();
        // expected.sort();
        // assert_eq!(expected, result);

        let mut expected2 = vec![
            "a:1 (af);b:2 (bf) 51000",
            "c:3 (cf) 234",
            "a:7 (af);b:2 (bf) 6000",
        ];
        let mut result2: Vec<String> = tracker.to_lines(true, false, same_callstack).collect();
        result2.sort();
        expected2.sort();
        assert_eq!(expected2, result2);
    }

    #[test]
    fn test_unknown_function_id() {
        let func_locations = VecFunctionLocations::new();
        let (function, filename) = func_locations.get_function_and_filename(FunctionId::UNKNOWN);
        assert_eq!(filename, "UNKNOWN DUE TO BUG");
        assert_eq!(function, "UNKNOWN");
    }

    // TODO test to_lines(false)
}
