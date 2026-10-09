use std::collections::HashMap;

#[derive(Clone, Copy)]
struct Interval {
    start: u64,
    end: u64,
}

pub(super) struct PreviousChunkIndex {
    by_text: HashMap<String, Vec<Interval>>,
}

impl PreviousChunkIndex {
    pub(super) fn new<'a>(words: impl Iterator<Item = (&'a str, u64, u64)>) -> Self {
        let mut by_text: HashMap<String, Vec<Interval>> = HashMap::new();
        for (text, start, end) in words.filter(|(_, start, end)| start < end) {
            by_text
                .entry(text.into())
                .or_default()
                .push(Interval { start, end });
        }
        for intervals in by_text.values_mut() {
            intervals.sort_unstable_by_key(|interval| (interval.start, interval.end));
            let mut merged: Vec<Interval> = Vec::with_capacity(intervals.len());
            for interval in intervals.drain(..) {
                match merged.last_mut() {
                    Some(previous) if interval.start < previous.end => {
                        previous.end = previous.end.max(interval.end);
                    }
                    Some(_) | None => merged.push(interval),
                }
            }
            *intervals = merged;
        }
        Self { by_text }
    }

    pub(super) fn overlaps(&self, text: &str, start: u64, end: u64) -> bool {
        if start >= end {
            return false;
        }
        let Some(intervals) = self.by_text.get(text) else {
            return false;
        };
        let index = intervals.partition_point(|interval| interval.start < end);
        index > 0 && start < intervals[index - 1].end
    }
}

#[cfg(test)]
mod tests {
    use super::PreviousChunkIndex;

    #[test]
    fn overlapping_positive_intervals_are_merged_for_logarithmic_lookup() {
        // Given
        let index = PreviousChunkIndex::new(
            [("word", 30, 50), ("word", 10, 40), ("word", 70, 80)].into_iter(),
        );
        // When
        let overlapping = index.overlaps("word", 45, 75);
        let touching = index.overlaps("word", 50, 70);
        // Then
        assert!(overlapping);
        assert!(!touching);
        assert_eq!(index.by_text["word"].len(), 2);
    }

    #[test]
    fn zero_duration_intervals_and_candidates_are_never_deduplicated() {
        // Given
        let index = PreviousChunkIndex::new([("word", 10, 20), ("word", 15, 15)].into_iter());
        // When
        let point = index.overlaps("word", 15, 15);
        // Then
        assert!(!point);
    }
}
