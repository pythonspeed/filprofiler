//! A line caching library, a bit like Python's linecache.

use ahash::RandomState;
use parking_lot::Mutex;
use pyo3::{prelude::*, types::PyString};
use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader, Cursor, Read},
    sync::Arc,
};

/// Store a cache of files in memory, allow easy reading of lines.
#[derive(Default)]
pub struct LineCacher {
    file_lines: HashMap<String, Vec<String>, RandomState>,
}

static EMPTY_STRING: String = String::new();

impl LineCacher {
    /// Add lines for a particular filename, if it's not already added, returning an Entry.
    fn maybe_add_file_data<'a, GR: FnOnce() -> Option<Box<dyn Read>>>(
        &'a mut self,
        filename: &str,
        get_data: GR,
    ) -> &'a Vec<String> {
        self.file_lines
            .entry(filename.to_string())
            .or_insert_with(|| {
                get_data()
                    .map(|r| {
                        BufReader::new(r)
                            .lines()
                            .map(|l| l.unwrap_or_else(|_| EMPTY_STRING.clone()))
                            .collect()
                    })
                    .unwrap_or_else(Vec::new)
            })
    }

    /// Add a string-of-lines for a particular filename.
    fn add_string(&mut self, filename: &str, lines: &str) {
        let lines = Cursor::new(lines.to_string().into_bytes());
        self.maybe_add_file_data(filename, || Some(Box::new(lines)));
    }

    /// Get the source code line for the given file. If the file doesn't exist
    /// or line number is too big, an empty string is returned. Line endings are
    /// stripped.
    pub fn get_source_line<'a>(&'a mut self, filename: &str, line_number: usize) -> &'a str {
        if line_number == 0 {
            return &EMPTY_STRING;
        }
        let entry = self.maybe_add_file_data(filename, || {
            File::open(filename).ok().map(|f| {
                let b: Box<dyn Read> = Box::new(f);
                b
            })
        });
        entry.get(line_number - 1).unwrap_or(&EMPTY_STRING)
    }
}

/// Wrapper around Python's linecache.cache dictionary that captures manual
/// writes.
///
/// In particular, IPython/Jupyter cells will get added there, and so we want to
/// make sure we have access to them in our linecache so we can create correct
/// tracebacks.
#[pyclass]
struct PyLineCacheWrapper {
    py_linecache: PyObject,
    rust_linecache: Arc<Mutex<LineCacher>>,
}

impl PyLineCacheWrapper {
    fn new(py_linecache: &PyAny) -> Self {
        // TODO populate Rust linecache with non-filesystem cached data.
        // TODO and write test.
        Self {
            py_linecache: py_linecache.into(),
            rust_linecache: Arc::new(Mutex::new(LineCacher::default())),
        }
    }
}

#[pymethods]
impl PyLineCacheWrapper {
    /// Pass through pretty much everything to the underyling dict:
    fn __getattribute__(slf: PyRef<'_, Self>, attr: &PyString) -> PyResult<Py<PyAny>> {
        slf.py_linecache.getattr(slf.py(), attr)
    }

    /// __setitem__ is only code we handle specially:
    fn __setitem__(slf: PyRef<'_, Self>, attr: &PyAny, value: &PyAny) -> PyResult<()> {
        // TODO if entry is not a file, added it to rust cache.
        // TODO and test.
        println!("ADDING {} to linecache", attr);
        let py = slf.py();
        slf.py_linecache
            .getattr(py, "__setitem__")?
            .call(py, (attr, value), None)?;
        Ok(())
    }
}

/// Make sure changes to the Python linecache end up in our cache, in particular
/// for Jupyter which manually adds item to Python's cache.
fn monkeypatch_python_linecache() -> PyResult<()> {
    // TODO write test showing this changes the global linecache.
    Python::with_gil(|py| {
        let linecache = py.import("linecache")?;
        let py_cache = linecache.getattr("cache")?;
        let wrapper = PyLineCacheWrapper::new(py_cache);
        linecache.setattr("cache", Py::new(py, wrapper)?)?;
        Ok(())
    })
}

/// Get access to the LineCacher that is attached to `linecache` Python module
/// after calling `monkeypatch_python_linecache()`.
///
/// If none is installed, a new one will be installed.
///
/// If it's impossible to install for some reason, a temporary one-off will be
/// created, since at least filesystem tracebacks can still be used.
pub fn run_with_linecache<F: FnOnce(&LineCacher)>(closure: F) {
    // TODO write test showing this gets existing object attached to global linecache.
    // TODO write test showing this attaches new one if global linecache is missing this object.
    fn get_wrapper(py: Python<'_>) -> PyResult<Arc<Mutex<LineCacher>>> {
        let linecache = py.import("linecache")?;
        let wrapper = linecache.getattr("cache")?;
        let wrapper: &PyCell<PyLineCacheWrapper> = wrapper.downcast()?;
        Ok(wrapper.try_borrow()?.rust_linecache.clone())
    }

    let linecacher = Python::with_gil(|py| {
        get_wrapper(py)
            .or_else(|_| {
                monkeypatch_python_linecache();
                get_wrapper(py)
            })
            .unwrap_or_else(|_| Arc::new(Mutex::new(LineCacher::default())))
    });
    closure(&linecacher.lock());
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusty_fork::rusty_fork_test;
    use std::io::Write;

    #[test]
    fn linecacher() {
        let mut cache = LineCacher::default();

        // Non-existent file
        assert_eq!(cache.get_source_line("/no/such/file", 1), "");

        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.as_file_mut().write_all(b"abc\ndef\r\nghijk").unwrap();
        let path = f.path().as_os_str().to_str().unwrap();

        // 0 line number
        assert_eq!(cache.get_source_line(path, 0), "");

        // Too high line number
        assert_eq!(cache.get_source_line(path, 4), "");

        // Present line numbers
        assert_eq!(cache.get_source_line(path, 1), "abc");
        assert_eq!(cache.get_source_line(path, 2), "def");
        assert_eq!(cache.get_source_line(path, 3), "ghijk");
    }

    #[test]
    fn linecacher_add_string() {
        let mut cache = LineCacher::default();
        cache.add_string("file1", "a\nb");
        cache.add_string("file2", "c\nd");
        assert_eq!(cache.get_source_line("file1", 1), "a");
        assert_eq!(cache.get_source_line("file1", 2), "b");
        assert_eq!(cache.get_source_line("file2", 1), "c");
        assert_eq!(cache.get_source_line("file2", 2), "d");
    }

    rusty_fork_test! {
        //#[test]
        //fn
    }
}
