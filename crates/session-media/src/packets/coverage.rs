use super::{Envelope, ProbeError};
use std::cmp::Reverse;

const REORDER_WINDOW: usize = 64;

impl Envelope {
    pub(super) fn queue_coverage(&mut self, start: i64, end: i64) -> Result<(), ProbeError> {
        self.pending.push(Reverse((start, end)));
        if self.pending.len() > REORDER_WINDOW {
            self.emit_coverage()?;
        }
        Ok(())
    }

    pub(super) fn finish_coverage(&mut self) -> Result<(), ProbeError> {
        while !self.pending.is_empty() {
            self.emit_coverage()?;
        }
        Ok(())
    }

    fn emit_coverage(&mut self) -> Result<(), ProbeError> {
        if let Some(Reverse((start, end))) = self.pending.pop() {
            if self.last_emitted.is_some_and(|previous| start < previous) {
                self.coverage_resolved = false;
            }
            self.last_emitted = Some(start);
            if let Some(frontier) = self.frontier {
                if start > frontier {
                    match start.checked_sub(frontier) {
                        Some(gap) => {
                            self.gap_count = self
                                .gap_count
                                .checked_add(1)
                                .ok_or(ProbeError::OutputLimit)?;
                            self.max_gap =
                                Some(self.max_gap.map_or(gap, |maximum| maximum.max(gap)));
                        }
                        None => self.coverage_resolved = false,
                    }
                } else if start < frontier {
                    self.overlap_count = self
                        .overlap_count
                        .checked_add(1)
                        .ok_or(ProbeError::OutputLimit)?;
                }
            }
            self.frontier = Some(self.frontier.map_or(end, |frontier| frontier.max(end)));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_window_resolves_reordering_without_retaining_the_recording() -> Result<(), ProbeError>
    {
        let mut envelope = Envelope::default();
        for pair in 0..10_000 {
            envelope.queue_coverage(pair * 2 + 1, pair * 2 + 2)?;
            envelope.queue_coverage(pair * 2, pair * 2 + 1)?;
            assert!(envelope.pending.len() <= REORDER_WINDOW);
        }
        envelope.finish_coverage()?;
        assert!(envelope.coverage_resolved);
        assert_eq!(envelope.gap_count, 0);
        Ok(())
    }

    #[test]
    fn reordering_beyond_the_window_is_explicitly_unverifiable() -> Result<(), ProbeError> {
        let mut envelope = Envelope::default();
        for start in 1..=70 {
            envelope.queue_coverage(start, start + 1)?;
        }
        envelope.queue_coverage(0, 1)?;
        envelope.finish_coverage()?;
        assert!(!envelope.coverage_resolved);
        Ok(())
    }
}
