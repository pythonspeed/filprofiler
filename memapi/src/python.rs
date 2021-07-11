// Interactions with Python APIs.
use once_cell::sync::Lazy;
use pyo3::prelude::*;
use pyo3::types::PyModule;

// Get the source code line from a given filename.
pub fn get_source_line(filename: &str, line_number: u16) -> PyResult<String> {
    Python::with_gil(|py| {
        let linecache = PyModule::import(py, "linecache")?;
        let result: String = linecache
            .call1("getline", (filename, line_number))?
            .to_string();
        Ok(result)
    })
}

// Return the filesystem path of the stdlib's runpy module.
pub fn get_runpy_path() -> &'static str {
    static path: Lazy<String> = Lazy::new(|| {
        Python::with_gil(|py| {
            let runpy = PyModule::import(py, "runpy").unwrap();
            runpy.filename().unwrap().to_string()
        })
    });
    path.as_str()
}
