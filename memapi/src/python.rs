// Interactions with Python APIs.
use once_cell::sync::Lazy;
use pyo3::prelude::*;
use pyo3::types::PyModule;

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
