//! A Rust wrapper around Python's linecache. We can't just emulate it because
//! PEP 302 `__loader__`s and ipython shoving stuff into it and oh god oh god oh
//! god Python is complicated.

use crate::python;

/// Wrapper around Python's linecache.
#[derive(Default)]
pub struct LineCacher {}

impl LineCacher {
    /// Get the source code line for the given file.
    pub fn get_source_line(&mut self, filename: &str, line_number: usize) -> String {
        if line_number == 0 {
            return String::new();
        }
        python::get_source_line(filename, line_number).unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pyo3::prelude::*;
    use rusty_fork::rusty_fork_test;
    use std::io::Write;

    rusty_fork_test! {
        /// The linecache can read files.
        #[test]
        fn linecacher_from_file() {
            pyo3::prepare_freethreaded_python();
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
            assert_eq!(cache.get_source_line(path, 1), "abc\n");
            assert_eq!(cache.get_source_line(path, 2), "def\n");
            assert_eq!(cache.get_source_line(path, 3), "ghijk\n");
        }

        /// The linecache can read random crap shoved into the linecache module.
        #[test]
        fn linecacher_from_arbitrary_source() {
            pyo3::prepare_freethreaded_python();
            let mut cache = LineCacher::default();

            Python::with_gil(|py| {
                let blah = vec!["arr\n", "boo"];
                let linecache = PyModule::import_bound(py, "linecache")?;
                linecache
                    .getattr("cache")?.set_item("blah", (8, 0, blah, "blah"))?;
                Ok::<(), PyErr>(())
            }).unwrap();

            assert_eq!(cache.get_source_line("blah", 1), "arr\n");
            assert_eq!(cache.get_source_line("blah", 2), "boo");
        }
    }
}
