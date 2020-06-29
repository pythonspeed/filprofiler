use libc;
use std::cmp::{max, min};
use std::collections::HashMap;

/// Open-ended range in memory, [A...B).
#[derive(Clone, Debug)]
struct Range {
    start: usize,
    end: usize,
}

impl Range {
    fn new(start: usize, length: libc::size_t) -> Self {
        assert!(length > 0);
        Range {
            start,
            end: start + length,
        }
    }

    fn intersection(&self, other: &Range) -> Option<Range> {
        let max_start = max(self.start, other.start);
        let min_end = min(self.end, other.end);
        if min_end <= max_start {
            None
        } else {
            Some(Range {
                start: max_start,
                end: min_end,
            })
        }
    }
}

/// Map from memory address range to some other object, typically a CallStack.
///
/// The intended use case is tracking anonymous mmap(), where munmap() can
/// deallocate chunks of an allocation, or even multiple allocations.
pub struct RangeMap<V: Clone> {
    ranges: Vec<(Range, V)>,
}

impl<V: Clone> RangeMap<V> {
    pub fn new() -> Self {
        RangeMap { ranges: vec![] }
    }

    pub fn add(&mut self, start: usize, length: libc::size_t, value: V) {
        if length <= 0 {
            return;
        }
        self.ranges.push((Range::new(start, length), value));
    }

    pub fn remove(&mut self, start: usize, length: libc::size_t) {
        if length <= 0 {
            return;
        }
        let mut new_ranges = vec![];
        let remove = Range::new(start, length);
        for (range, value) in self.ranges.iter() {
            match range.intersection(&remove) {
                // Total overlap, remove it all:
                Some(i) if (i.start == range.start) && (i.end == range.end) => (),
                // Remove chunk from start:
                Some(i) if (i.start == range.start) && (i.end < range.end) => {
                    new_ranges.push((
                        Range {
                            start: i.end,
                            end: range.end,
                        },
                        value.clone(),
                    ));
                }
                // Remove chunk from end:
                Some(i) if (i.start > range.start) && (i.end == range.end) => {
                    new_ranges.push((
                        Range {
                            start: range.start,
                            end: i.start,
                        },
                        value.clone(),
                    ));
                }
                // Remove chunk from the middle:
                Some(i) => {
                    new_ranges.push((
                        Range {
                            start: range.start,
                            end: i.start,
                        },
                        value.clone(),
                    ));
                    new_ranges.push((
                        Range {
                            start: i.end,
                            end: range.end,
                        },
                        value.clone(),
                    ));
                }
                // No overlap, remove nothing:
                None => {
                    new_ranges.push((range.clone(), value.clone()));
                }
            }
        }
        self.ranges = new_ranges;
    }

    pub fn as_hashmap(&self) -> HashMap<usize, &V> {
        self.ranges
            .iter()
            .map(|(range, v)| (range.start, v))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::RangeMap;
    use proptest::prelude::*;
    use std::collections::{BTreeMap, HashMap};

    /// RangeMap that has a key for each address in the range, rather than a
    /// smarter technique involving ranges.
    struct StupidRangeMap<V> {
        items: BTreeMap<usize, V>,
    }

    impl<V: PartialEq + Clone> StupidRangeMap<V> {
        fn new() -> Self {
            StupidRangeMap {
                items: BTreeMap::new(),
            }
        }

        fn add(&mut self, start: usize, length: libc::size_t, value: V) {
            assert!(length > 0);
            for i in start..(start + length) {
                self.items.insert(i, value.clone());
            }
        }

        fn remove(&mut self, start: usize, length: libc::size_t) {
            assert!(length > 0);
            for i in start..(start + length) {
                self.items.remove(&i);
            }
        }

        fn as_hashmap(&self) -> HashMap<usize, &V> {
            let mut result = HashMap::new();
            let mut previous_address = 0;
            let mut previous_value = None;
            for (k, v) in self.items.iter() {
                if (*k == previous_address + 1) && (previous_value == Some(v)) {
                    previous_address = *k;
                    continue;
                } else {
                    previous_address = *k;
                    previous_value = Some(v);
                    result.insert(*k, v);
                }
            }
            result
        }
    }

    fn ranges() -> impl Strategy<Value = Vec<(usize, usize)>> {
        proptest::collection::vec((1..20usize, 1..20usize), 100)
            .prop_map(|vec| {
                let mut result: Vec<(usize, usize)> = Vec::new();
                let mut previous_start = 0 as usize;
                for (shift_start, length) in vec.iter() {
                    previous_start += shift_start;
                    result.push((previous_start, *length));
                    previous_start += length;
                }
                result
            })
            .prop_shuffle()
            .boxed()
    }

    proptest! {
        /// We can add ranges and get the same result in the real and stupid range
        /// maps.
        #[test]
        fn adding_ranges(ranges in ranges()) {
            let mut real_rangemap : RangeMap<usize> = RangeMap::new();
            let mut stupid_rangemap: StupidRangeMap<usize> = StupidRangeMap::new();
            for (start, length) in ranges {
                real_rangemap.add(start, length, start * (length as usize));
                stupid_rangemap.add(start, length, start * (length as usize));
                prop_assert_eq!(real_rangemap.as_hashmap(), stupid_rangemap.as_hashmap());
            }
        }

        /// We can remove ranges and get the same result in the real and stupid
        /// range maps.
        #[test]
        fn removing_ranges(add_ranges in ranges(), remove_ranges in ranges()) {
            let mut real_rangemap : RangeMap<usize> = RangeMap::new();
            let mut stupid_rangemap: StupidRangeMap<usize> = StupidRangeMap::new();
            for (start, length) in add_ranges {
                real_rangemap.add(start, length, start * (length as usize));
                stupid_rangemap.add(start, length, start * (length as usize));
                prop_assert_eq!(real_rangemap.as_hashmap(), stupid_rangemap.as_hashmap());
            }
            for (start, length) in remove_ranges {
                real_rangemap.remove(start, length);
                stupid_rangemap.remove(start, length);
                prop_assert_eq!(real_rangemap.as_hashmap(), stupid_rangemap.as_hashmap());
            }
        }
    }
}
