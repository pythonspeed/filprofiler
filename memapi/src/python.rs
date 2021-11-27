// Interactions with Python APIs.
use once_cell::sync::Lazy;
use pyo3::prelude::*;
use pyo3::types::PyModule;

/// Get the source code line from a given filename.
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

/// Return the filesystem path of the stdlib's runpy module.
pub fn get_runpy_path() -> &'static str {
    static PATH: Lazy<String> = Lazy::new(|| {
        Python::with_gil(|py| {
            let runpy = PyModule::import(py, "runpy").unwrap();
            runpy.filename().unwrap().to_string()
        })
    });
    PATH.as_str()
}

/// Strip sys.path prefixes from Python modules' pathes.
pub struct PrefixStripper {
    prefixes: Vec<String>,
}

impl PrefixStripper {
    pub fn new() -> Self {
        let prefixes = Python::with_gil(|py| {
            let paths = py.eval(
                // 1. Drop non-string values, they're not something we can understand.
                // 2. Drop empty string, it's misleading.
                // 3. Add '/' to end of all paths.
                "[__import__('os').path.normpath(path) + '/' for path in __import__('sys').path if (isinstance(path, str) and path)]",
                None,
                None,
            );
            paths
                .map(|p| p.extract::<Vec<String>>().unwrap_or_else(|_| vec![]))
                .unwrap_or_else(|_| vec![])
        });
        PrefixStripper { prefixes }
    }

    /// Remove the sys.path prefix from a path to an imported module.
    ///
    /// E.g. if the input is "/usr/lib/python3.9/threading.py", the result will
    /// probably be "threading.py".
    pub fn strip_prefix<'a>(&self, path: &'a str) -> &'a str {
        for prefix in &self.prefixes {
            if path.starts_with(prefix) {
                return &path[prefix.len()..path.len()];
            }
        }
        // No prefix found.
        path
    }
}
