use std::{borrow::Cow, fs, io::Write, path::Path};

use inferno::flamegraph;
use itertools::Itertools;

use crate::{
    linecache::LineCacher,
    memorytracking::{Callstack, FunctionLocations},
};

/// Filter down to top 99% of samples.
///
/// 1. Empty samples are dropped.
/// 2. Top 99% of samples, starting with largest, are kept.
/// 3. If that's less than 100 samples, thrown in up to 100, main goal is
///    just to not have a vast number of useless tiny allocations.
pub fn filter_to_useful_callstacks<'i, K, I>(
    samples: I,
    total_samples: usize,
) -> impl Iterator<Item = (K, usize)>
where
    K: Eq + std::hash::Hash + 'i,
    I: Iterator<Item = (K, &'i usize)>,
{
    samples
        .map(|(k, v)| (k, *v))
        // Filter out callstacks with no samples:
        .filter(|(_, size)| *size > 0)
        // Sort in descending size of sample:
        .sorted_by(|a, b| Ord::cmp(&b.1, &a.1))
        // Keep track of how much total samples we've accumulated so far:
        .scan(0usize, |stored, (i, size)| {
            *stored += size;
            Some((*stored, i, size))
        })
        // We don't do more than 10,000 samples. More than that uses vast
        // amounts of memory to generate the report, and overburdens the browser
        // displaying the SVG.
        .take(10_000)
        // Stop once we've hit 99% of samples, but include at least 100 just
        // so there's some context:
        .scan(
            (false, 0),
            move |(past_threshold, taken), (stored, i, size)| {
                if *past_threshold && (*taken > 99) {
                    return None;
                }
                // Stop if we've hit 99% of allocated data.
                *past_threshold = stored > (total_samples * 99) / 100;
                *taken += 1;
                Some((i, size))
            },
        )
}

/// Write strings to disk, one line per string.
pub fn write_lines<I: IntoIterator<Item = String>>(lines: I, path: &Path) -> std::io::Result<()> {
    let mut file = std::fs::File::create(path)?;
    for line in lines {
        file.write_all(line.as_bytes())?;
        file.write_all(b"\n")?;
    }
    file.flush()?;
    Ok(())
}

/// A strategy for cleaning up callstacks before rendering them to text.
pub trait CallstackCleaner {
    fn cleanup<'a>(&self, callstack: &'a Callstack) -> Cow<'a, Callstack>;
}

/// The data needed to create a flamegraph.
pub struct FlamegraphCallstacks<D, FL: FunctionLocations, UC> {
    data: D,
    functions: FL,
    callstack_cleaner: UC,
}

impl<'a, D, FL, UC> FlamegraphCallstacks<D, FL, UC>
where
    &'a D: IntoIterator<Item = (&'a Callstack, &'a usize)>,
    <&'a D as IntoIterator>::IntoIter: ExactSizeIterator,
    D: 'a,
    FL: FunctionLocations,
    UC: CallstackCleaner,
{
    pub fn new(data: D, functions: FL, callstack_cleaner: UC) -> Self {
        Self {
            data,
            functions,
            callstack_cleaner,
        }
    }

    /// Create iterator over the line-based string format parsed by the inferno
    /// crate.
    pub fn to_lines(
        &'a self,
        to_be_post_processed: bool,
    ) -> impl ExactSizeIterator<Item = String> + 'a {
        let by_call = (&self.data).into_iter();
        let mut linecache = LineCacher::default();
        by_call.map(move |(callstack, size)| {
            format!(
                "{} {}",
                self.callstack_cleaner.cleanup(callstack).as_string(
                    to_be_post_processed,
                    &self.functions,
                    ";",
                    &mut linecache,
                ),
                size,
            )
        })
    }

    /// Low-level interface for writing flamegraphs with post-processing:
    pub fn get_flamegraph_with_options(
        &'a self,
        to_be_post_processed: bool,
        options: flamegraph::Options,
        // special cased because it needs to be psot-processed:
        subtitle: Option<&str>,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        let lines = self.to_lines(to_be_post_processed);
        get_flamegraph_with_options(lines, to_be_post_processed, options, subtitle)
    }

    /// Write a flamegraph SVG to disk, given lines in summarized format.
    pub fn get_flamegraph(
        &'a self,
        reversed: bool,
        title: &str,
        subtitle: &str,
        count_name: &str,
        to_be_post_processed: bool,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        get_flamegraph(
            self.to_lines(to_be_post_processed),
            reversed,
            title,
            subtitle,
            count_name,
            to_be_post_processed,
        )
    }

    /// Write a flamegraph SVG to disk.
    pub fn write_flamegraph(
        &'a self,
        path: &Path,
        reversed: bool,
        title: &str,
        subtitle: &str,
        count_name: &str,
        to_be_post_processed: bool,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let flamegraph =
            self.get_flamegraph(reversed, title, subtitle, count_name, to_be_post_processed)?;
        let mut file = std::fs::File::create(path)?;
        file.write_all(&flamegraph)?;
        Ok(())
    }

    /// Write .prof, -source.prof, .svg and -reversed.svg files for given lines.
    pub fn write_flamegraphs(
        &'a self,
        directory_path: &Path,
        base_filename: &str,
        title: &str,
        subtitle: &str,
        count_name: &str,
        to_be_post_processed: bool,
    ) {
        if !directory_path.exists() {
            fs::create_dir_all(directory_path)
                .expect("=fil-profile= Couldn't create the output directory.");
        } else if !directory_path.is_dir() {
            panic!("=fil-profile= Output path must be a directory.");
        }

        let raw_path_without_source_code = directory_path.join(format!("{}.prof", base_filename));

        let raw_path_with_source_code =
            directory_path.join(format!("{}-source.prof", base_filename));

        // Always write .prof file without source code, for use by tests and
        // other automated post-processing.
        if let Err(e) = write_lines(self.to_lines(false), &raw_path_without_source_code) {
            eprintln!("=fil-profile= Error writing raw profiling data: {}", e);
            return;
        }

        // Optionally write version with source code for SVGs, if we're using
        // source code.
        if to_be_post_processed {
            if let Err(e) = write_lines(self.to_lines(true), &raw_path_with_source_code) {
                eprintln!("=fil-profile= Error writing raw profiling data: {}", e);
                return;
            }
        }

        let svg_path = directory_path.join(format!("{}.svg", base_filename));
        match self.write_flamegraph(
            &svg_path,
            false,
            title,
            subtitle,
            count_name,
            to_be_post_processed,
        ) {
            Ok(_) => {
                eprintln!("=fil-profile= Wrote flamegraph to {:?}", svg_path);
            }
            Err(e) => {
                eprintln!("=fil-profile= Error writing SVG: {}", e);
            }
        }
        let svg_path = directory_path.join(format!("{}-reversed.svg", base_filename));
        match self.write_flamegraph(
            &svg_path,
            true,
            title,
            subtitle,
            count_name,
            to_be_post_processed,
        ) {
            Ok(_) => {
                eprintln!("=fil-profile= Wrote flamegraph to {:?}", svg_path);
            }
            Err(e) => {
                eprintln!("=fil-profile= Error writing SVG: {}", e);
            }
        }
        if to_be_post_processed {
            // Don't need this file, and it'll be quite big, so delete it.
            let _ = std::fs::remove_file(raw_path_with_source_code);
        }
    }
}

/// Low-level interface for writing flamegraphs with post-processing:
pub fn get_flamegraph_with_options<I: IntoIterator<Item = String>>(
    lines: I,
    to_be_post_processed: bool,
    mut options: flamegraph::Options,
    // special cased because it needs to be psot-processed:
    subtitle: Option<&str>,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut output = vec![];
    let lines: Vec<String> = lines.into_iter().collect();
    match flamegraph::from_lines(&mut options, lines.iter().map(|s| s.as_ref()), &mut output) {
        Err(e) => Err(format!("{}", e).into()),
        Ok(_) => {
            if to_be_post_processed {
                // Replace with real subtitle.
                let data = String::from_utf8(output)?;
                let data = if let Some(subtitle) = subtitle {
                    data.replace("__FIL-SUBTITLE-HERE__", subtitle)
                } else {
                    data
                };
                // Restore normal semi-colons.
                let data = data.replace('\u{ff1b}', ";");
                // Restore (non-breaking) spaces.
                let data = data.replace('\u{12e4}', "\u{00a0}");
                // Get rid of empty-line markers:
                let data = data.replace('\u{2800}', "");
                output = data.as_bytes().to_vec();
            }
            Ok(output)
        }
    }
}

/// Write a flamegraph SVG to disk, given lines in summarized format.
pub fn get_flamegraph<I: IntoIterator<Item = String>>(
    lines: I,
    reversed: bool,
    title: &str,
    subtitle: &str,
    count_name: &str,
    to_be_post_processed: bool,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let title = format!("{}{}", title, if reversed { ", Reversed" } else { "" },);
    let mut options = flamegraph::Options::default();
    options.title = title;
    options.count_name = count_name.to_string();
    options.font_size = 16;
    options.font_type = "monospace".to_string();
    options.frame_height = 22;
    options.reverse_stack_order = reversed;
    options.color_diffusion = true;
    options.direction = flamegraph::Direction::Inverted;
    options.min_width = 0.2;
    // Maybe disable this some day; but for now it makes debugging much
    // easier:
    options.pretty_xml = true;
    if to_be_post_processed {
        // Can't put structured text into subtitle, so have to do a hack.
        options.subtitle = Some("__FIL-SUBTITLE-HERE__".to_string());
    }
    get_flamegraph_with_options(lines, to_be_post_processed, options, Some(subtitle))
}

#[cfg(test)]
mod tests {
    use super::filter_to_useful_callstacks;
    use im::HashMap;
    use itertools::Itertools;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn filtering_of_callstacks(
            // Allocated bytes. Will use index as the memory address.
            allocated_sizes in prop::collection::vec(0..1000usize, 5..15000),
        ) {
            let total_size : usize = allocated_sizes.iter().sum();
            let total_size_99 = (99 * total_size) / 100;
            let callstacks = allocated_sizes.iter().enumerate();
            let filtered : HashMap<usize,usize>  = filter_to_useful_callstacks(callstacks, total_size).collect();
            let filtered_size :usize = filtered.values().into_iter().sum();
            if filtered_size >= total_size_99  {
                if filtered.len() > 100 {
                    // Removing any item should take us to or below 99%
                    for value in filtered.values() {
                        prop_assert!(filtered_size - *value <= total_size_99)
                    }
                }
            } else {
                // Cut out before 99%, so must be too many items
                prop_assert_eq!(filtered.len(), 10000);
                prop_assert_eq!(filtered_size, allocated_sizes.clone().iter().sorted_by(
                    |a, b| Ord::cmp(b, a)).take(10000).sum::<usize>());
            }
        }

    }
}
