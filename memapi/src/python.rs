// Interactions with Python APIs.
use once_cell::sync::Lazy;
use pyo3::ffi::PyCodeObject;
use pyo3::ffi::PyFrameObject;
use pyo3::ffi::PyFrame_GetLineNumber;
use pyo3::prelude::*;
use pyo3::types::PyModule;

use crate::memorytracking::{CallSiteId, Callstack, FunctionId, LineNumber};

// Get the source code line from a given filename.
pub fn get_source_line(filename: &str, line_number: u16) -> PyResult<String> {
    Python::with_gil(|py| {
        let linecache = PyModule::import(py, "linecache")?;
        let result: String = linecache
            .getattr("getline")?
            .call1((filename, line_number))?
            .to_string();
        Ok(result)
    })
}

// Return the filesystem path of the stdlib's runpy module.
pub fn get_runpy_path() -> &'static str {
    static PATH: Lazy<String> = Lazy::new(|| {
        Python::with_gil(|py| {
            let runpy = PyModule::import(py, "runpy").unwrap();
            runpy.filename().unwrap().to_string()
        })
    });
    PATH.as_str()
}

// Get the callstack for the given frame.
pub fn get_callstack<F>(mut frame: *mut PyFrameObject, get_function_id: F) -> Callstack
where
    F: Fn(*mut PyCodeObject) -> Option<FunctionId>,
{
    let mut result = vec![];

    // Starting with current frame, go up the stack and add info about parent
    // callers:
    while !frame.is_null() {
        let (function_id, line_number) = unsafe {
            let code = (*frame).f_code;
            (
                get_function_id(code).unwrap_or(FunctionId::UNKNOWN),
                PyFrame_GetLineNumber(frame),
            )
        };
        let line_number = line_number as LineNumber;
        result.push(CallSiteId::new(function_id, line_number));
        frame = unsafe { (*frame).f_back };
    }
    // Make sure callstack starts with top-most frame:
    result.reverse();
    Callstack::from_vec(result)
}
