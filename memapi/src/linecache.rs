//! A line caching library, a bit like Python's linecache.

use std::{
    collections::HashMap,
    fs::File,
    io::{BufRead, BufReader},
};

use ahash::RandomState;

/// Store a cache of files in memory, allow easy reading of lines.
#[derive(Default)]
pub struct LineCacher {
    file_lines: HashMap<String, Vec<String>, RandomState>,
}

static EMPTY_STRING: String = String::new();

impl LineCacher {
    /// Get the source code line for the given file. If the file doesn't exist
    /// or line number is too big, an empty string is returned. Line endings are
    /// stripped.
    pub fn get_source_line<'a>(&'a mut self, filename: &str, line_number: usize) -> &'a str {
        if line_number == 0 {
            return &EMPTY_STRING;
        }
        let entry =
            self.file_lines
                .entry(filename.to_string())
                .or_insert_with(|| -> Vec<String> {
                    File::open(filename)
                        .map(|f| {
                            BufReader::new(f)
                                .lines()
                                .map(|l| l.unwrap_or_else(|_| EMPTY_STRING.clone()))
                                .collect()
                        })
                        .unwrap_or_else(|_| vec![])
                });
        entry.get(line_number - 1).unwrap_or(&EMPTY_STRING)
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::LineCacher;

    #[test]
    fn linecache() {
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
