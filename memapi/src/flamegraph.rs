use std::{
    io::{Read, Seek, Write},
    path::{Path, PathBuf},
};

use inferno::flamegraph;
use itertools::Itertools;

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
pub fn write_lines<I: Iterator<Item = String>>(lines: I, path: &Path) -> std::io::Result<()> {
    let mut file = std::fs::File::create(path)?;
    for line in lines {
        file.write_all(line.as_bytes())?;
        file.write_all(b"\n")?;
    }
    file.flush()?;
    Ok(())
}

/// Write a flamegraph SVG to disk, given lines in summarized format.
pub fn write_flamegraph(
    lines_file_path: &str,
    path: &Path,
    reversed: bool,
    title: &str,
    to_be_post_processed: bool,
) -> std::io::Result<()> {
    let mut file = std::fs::File::create(path)?;
    let title = format!("{}{}", title, if reversed { ", Reversed" } else { "" },);
    let mut options = flamegraph::Options::default();
    options.title = title;
    options.count_name = "bytes".to_string();
    options.font_size = 16;
    options.font_type = "monospace".to_string();
    options.frame_height = 22;
    options.reverse_stack_order = reversed;
    options.color_diffusion = true;
    options.direction = flamegraph::Direction::Inverted;
    // Maybe disable this some day; but for now it makes debugging much
    // easier:
    options.pretty_xml = true;
    if to_be_post_processed {
        // Can't put structured text into subtitle, so have to do a hack.
        options.subtitle = Some("FIL-SUBTITLE-HERE".to_string());
    }
    match flamegraph::from_files(&mut options, &[PathBuf::from(lines_file_path)], &file) {
        Err(e) => Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            format!("{}", e),
        )),
        Ok(_) => {
            file.flush()?;
            if to_be_post_processed {
                // Replace with real subtitle.
                let mut file2 = std::fs::File::open(path)?;
                let mut data = String::new();
                file2.read_to_string(&mut data)?;
                let data = data.replace("FIL-SUBTITLE-HERE", r#"Made with the Fil profiler. <a href="https://pythonspeed.com/fil/" style="text-decoration: underline;" target="_parent">Try it on your code!</a>"#);
                // Restore normal semi-colons.
                let data = data.replace("\u{ff1b}", ";");
                // Restore (non-breaking) spaces.
                let data = data.replace("\u{12e4}", "\u{00a0}");
                // Get rid of empty-line markers:
                let data = data.replace("\u{2800}", "");
                file.seek(std::io::SeekFrom::Start(0))?;
                file.set_len(0)?;
                file.write_all(&data.as_bytes())?;
            }
            Ok(())
        }
    }
}
