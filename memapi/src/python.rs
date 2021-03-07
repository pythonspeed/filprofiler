// Interactions with Python APIs.
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
