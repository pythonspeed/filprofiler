//! Interactions with Python APIs.

use once_cell::sync::Lazy;
use pyo3::prelude::*;

// Get the source code line from a given filename.
pub fn get_source_line(filename: &str, line_number: usize) -> PyResult<String> {
    Python::with_gil(|py| {
        let linecache = PyModule::import_bound(py, "linecache")?;
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
            let runpy = PyModule::import_bound(py, "runpy").unwrap();
            runpy.filename().unwrap().to_string()
        })
    });
    PATH.as_str()
}
