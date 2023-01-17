//! A line caching library, a bit like Python's linecache.
//!
//! We don't want to use Python's linecache because calling back into Python for
//! that can result in deadlocks.

use crate::python;
use ahash::RandomState;
use std::collections::HashMap;

/// Wrapper around Python's linecache, with extra caching so we can call Python
/// less.
#[derive(Default)]
pub struct LineCacher {
    linecache: HashMap<(String, usize), String, RandomState>,
}

impl LineCacher {
    /// Get the source code line for the given file.
    pub fn get_source_line(&mut self, filename: &str, line_number: usize) -> String {
        if line_number == 0 {
            return String::new();
        }
        let entry = self
            .linecache
            .entry((filename.to_string(), line_number))
            .or_insert_with(|| {
                python::get_source_line(&filename, line_number).unwrap_or(String::new())
            });
        entry.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
            assert_eq!(cache.get_source_line(path, 1), "abc");
            assert_eq!(cache.get_source_line(path, 2), "def");
            assert_eq!(cache.get_source_line(path, 3), "ghijk");
        }
    }
}
