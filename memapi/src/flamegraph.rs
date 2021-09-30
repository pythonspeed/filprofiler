use std::{io::Write, path::Path};

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
