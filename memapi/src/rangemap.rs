use std::cmp::{max, min};
#[cfg(test)]
use std::collections::HashMap;

/// Open-ended range in memory, [A...B).
#[derive(Clone, Debug, PartialEq)]
struct Range {
    start: usize,
    end: usize,
}

impl Range {
    fn new(start: usize, length: usize) -> Self {
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

    fn size(&self) -> usize {
        self.end - self.start
    }
}

/// Map from memory address range to some other object, typically a CallStack.
///
/// The intended use case is tracking anonymous mmap(), where munmap() can
/// deallocate chunks of an allocation, or even multiple allocations.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct RangeMap<V: Clone> {
    ranges: Vec<(Range, V)>,
}

impl<V: Clone> RangeMap<V> {
    pub fn new() -> Self {
        RangeMap { ranges: vec![] }
    }

    pub fn add(&mut self, start: usize, length: usize, value: V) {
        if length <= 0 {
            return;
        }
        self.ranges.push((Range::new(start, length), value));
    }

    /// Return how many bytes were removed.
    pub fn remove(&mut self, start: usize, length: usize) -> Vec<(V, usize)> {
        if length <= 0 {
            return vec![];
        }
        let mut new_ranges = vec![];
        let mut removed = vec![];
        let remove = Range::new(start, length);
        for (range, value) in self.ranges.iter() {
            match range.intersection(&remove) {
                // Total overlap, remove it all:
                Some(i) if (i.start == range.start) && (i.end == range.end) => {
                    removed.push((value.clone(), range.size()));
                }
                // Remove chunk from start:
                Some(i) if (i.start == range.start) && (i.end < range.end) => {
                    let new_range = Range {
                        start: i.end,
                        end: range.end,
                    };
                    removed.push((value.clone(), range.size() - new_range.size()));
                    new_ranges.push((new_range, value.clone()));
                }
                // Remove chunk from end:
                Some(i) if (i.start > range.start) && (i.end == range.end) => {
                    let new_range = Range {
                        start: range.start,
                        end: i.start,
                    };
                    removed.push((value.clone(), range.size() - new_range.size()));
                    new_ranges.push((new_range, value.clone()));
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
                    removed.push((value.clone(), length));
                }
                // No overlap, remove nothing:
                None => {
                    new_ranges.push((range.clone(), value.clone()));
                }
            }
        }
        self.ranges = new_ranges;
        removed
    }

    pub fn size(&self) -> usize {
        self.ranges.iter().map(|(r, _)| r.size()).sum()
    }

    /// Return iterator of (length, value).
    pub fn into_iter(self) -> impl Iterator<Item = (usize, V)> {
        self.ranges.into_iter().map(|(r, v)| (r.size(), v))
    }

    #[cfg(test)]
    pub fn as_hashmap(&self) -> HashMap<usize, (usize, &V)> {
        self.ranges
            .iter()
            .map(|(range, v)| (range.start, (range.size(), v)))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::RangeMap;
    use proptest::prelude::*;
    use std::collections::{BTreeMap, HashMap};
    use std::hash::Hash;

    /// RangeMap that has a key for each address in the range, rather than a
    /// smarter technique involving ranges.
    struct StupidRangeMap<V> {
        items: BTreeMap<usize, V>,
    }

    impl<V: Copy + PartialEq + Clone + std::fmt::Debug + Eq + Hash + Ord> StupidRangeMap<V> {
        fn new() -> Self {
            StupidRangeMap {
                items: BTreeMap::new(),
            }
        }

        fn add(&mut self, start: usize, length: usize, value: V) {
            assert!(length > 0);
            for i in start..(start + length) {
                self.items.insert(i, value.clone());
            }
        }

        fn remove(&mut self, start: usize, length: usize) -> Vec<(V, usize)> {
            assert!(length > 0);
            let mut removed = HashMap::new();
            for i in start..(start + length) {
                if let Some(value) = self.items.remove(&i) {
                    *removed.entry(value).or_insert(0) += 1;
                }
            }
            removed.iter().map(|(k, v)| (*k, *v)).collect()
        }

        pub fn size(&self) -> usize {
            self.items.len()
        }

        fn as_hashmap(&self) -> HashMap<usize, (usize, &V)> {
            let mut result = HashMap::new();
            let mut last_entry: Option<&mut (usize, &V)> = None;
            let mut last_k: usize = 0;
            for (k, v) in self.items.iter() {
                match last_entry {
                    None => {
                        // Nothing at previous address at all:
                        result.insert(*k, (1, v));
                        last_entry = result.get_mut(k);
                    }
                    Some((size, value)) if (*value == v) && (*k == last_k + 1) => {
                        // Previous address exists, with same value:
                        *size += 1;
                    }
                    Some(_) => {
                        // Previous value either not adjacent or different value:
                        result.insert(*k, (1, v));
                        last_entry = result.get_mut(k);
                    }
                }
                last_k = *k;
            }
            result
        }
    }

    fn ranges() -> impl Strategy<Value = Vec<(usize, usize)>> {
        proptest::collection::vec((1..20usize, 1..20usize), 1..20)
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
        /// We can add and remove ranges and get the same result in the real and
        /// stupid range maps.
        #[test]
        fn adding_removing_ranges(add_ranges in ranges(), remove_ranges in ranges()) {
            let mut real_rangemap : RangeMap<usize> = RangeMap::new();
            let mut stupid_rangemap: StupidRangeMap<usize> = StupidRangeMap::new();
            for (start, length) in add_ranges {
                real_rangemap.add(start, length, start * (length as usize));
                stupid_rangemap.add(start, length, start * (length as usize));
                prop_assert_eq!(real_rangemap.size(), stupid_rangemap.size());
                prop_assert_eq!(real_rangemap.as_hashmap(), stupid_rangemap.as_hashmap());
            }
            for (start, length) in remove_ranges {
                let removed1 = real_rangemap.remove(start, length * 2);
                let removed2 = stupid_rangemap.remove(start, length * 2);
                let mut removed1_map = HashMap::new();
                for (k, v) in removed1 {
                    *removed1_map.entry(k).or_insert(0) += v;
                }
                let removed2_map : HashMap<usize, usize> = removed2.iter().map(|v| *v).collect();
                prop_assert_eq!(removed1_map, removed2_map);
                prop_assert_eq!(real_rangemap.size(), stupid_rangemap.size());
                prop_assert_eq!(real_rangemap.as_hashmap(), stupid_rangemap.as_hashmap());
            }
        }
    }
}
