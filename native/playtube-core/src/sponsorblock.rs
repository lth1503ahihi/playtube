/// High-performance Interval Tree / Disjoint Segment Engine for SponsorBlock
/// Replaces slow O(N) Java linear scanning with O(log N) binary interval lookup.

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SponsorSegment {
    pub start_ms: u64,
    pub end_ms: u64,
    pub category: u8,
    pub action: u8, // 1: Auto-skip, 2: Show prompt / Notice
}

#[derive(Debug, Default, Clone)]
pub struct SponsorTimeline {
    segments: Vec<SponsorSegment>,
    /// Prefix maximum of `end_ms` up to index `i`, enabling exact O(log N + K)
    /// interval queries even when unmerged segments of different categories/actions overlap.
    prefix_max_end: Vec<u64>,
}

impl SponsorTimeline {
    pub fn new() -> Self {
        Self {
            segments: Vec::new(),
            prefix_max_end: Vec::new(),
        }
    }

    /// Sets and normalizes sponsor segments: sorts by start time and merges overlapping
    /// or adjacent segments that share the same category and action.
    pub fn set_segments(&mut self, mut list: Vec<SponsorSegment>) {
        if list.is_empty() {
            self.segments.clear();
            self.prefix_max_end.clear();
            return;
        }

        // Sort by start_ms, then end_ms
        list.sort_by(|a, b| a.start_ms.cmp(&b.start_ms).then(a.end_ms.cmp(&b.end_ms)));

        let mut merged: Vec<SponsorSegment> = Vec::with_capacity(list.len());
        for seg in list {
            if seg.start_ms >= seg.end_ms {
                continue; // invalid segment
            }

            if let Some(last) = merged.last_mut() {
                // Merge only when overlapping/adjacent AND category + action match
                if seg.start_ms <= last.end_ms
                    && seg.category == last.category
                    && seg.action == last.action
                {
                    last.end_ms = last.end_ms.max(seg.end_ms);
                    continue;
                }
            }
            merged.push(seg);
        }

        let mut prefix_max_end = Vec::with_capacity(merged.len());
        let mut max_end = 0u64;
        for seg in &merged {
            max_end = max_end.max(seg.end_ms);
            prefix_max_end.push(max_end);
        }

        self.segments = merged;
        self.prefix_max_end = prefix_max_end;
    }

    /// Interval query: determines if pos_ms falls inside any segment.
    /// Returns Some((target_skip_pos_ms, category, action)) if inside a segment, or None otherwise.
    /// When multiple overlapping segments contain pos_ms, prefers auto-skip (action == 1) and the
    /// furthest end_ms.
    #[inline]
    pub fn check_position(&self, pos_ms: u64) -> Option<(u64, u8, u8)> {
        if self.segments.is_empty() {
            return None;
        }

        // All segments with start_ms <= pos_ms are in 0..idx
        let idx = self.segments.partition_point(|s| s.start_ms <= pos_ms);
        if idx == 0 {
            return None;
        }

        let mut best: Option<(u64, u8, u8)> = None;
        for i in (0..idx).rev() {
            if self.prefix_max_end[i] <= pos_ms {
                break; // No segment at <= i can reach pos_ms
            }
            let seg = &self.segments[i];
            if pos_ms < seg.end_ms {
                match best {
                    None => best = Some((seg.end_ms, seg.category, seg.action)),
                    Some((best_end, _, best_act)) => {
                        let prefer_new = (seg.action == 1 && best_act != 1)
                            || (seg.action == best_act && seg.end_ms > best_end);
                        if prefer_new {
                            best = Some((seg.end_ms, seg.category, seg.action));
                        }
                    }
                }
            }
        }

        best
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.segments.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_merge_and_check() {
        let mut timeline = SponsorTimeline::new();
        timeline.set_segments(vec![
            SponsorSegment { start_ms: 10_000, end_ms: 20_000, category: 1, action: 1 },
            SponsorSegment { start_ms: 18_000, end_ms: 25_000, category: 1, action: 1 }, // overlapping
            SponsorSegment { start_ms: 40_000, end_ms: 50_000, category: 2, action: 1 },
        ]);

        assert_eq!(timeline.len(), 2);
        assert_eq!(timeline.segments[0].start_ms, 10_000);
        assert_eq!(timeline.segments[0].end_ms, 25_000);

        // Check positions
        assert_eq!(timeline.check_position(5_000), None);
        assert_eq!(timeline.check_position(10_000), Some((25_000, 1, 1)));
        assert_eq!(timeline.check_position(20_000), Some((25_000, 1, 1)));
        assert_eq!(timeline.check_position(24_999), Some((25_000, 1, 1)));
        assert_eq!(timeline.check_position(25_000), None);
        assert_eq!(timeline.check_position(45_000), Some((50_000, 2, 1)));
        assert_eq!(timeline.check_position(60_000), None);
    }

    #[test]
    fn test_overlapping_different_categories_not_merged() {
        let mut timeline = SponsorTimeline::new();
        timeline.set_segments(vec![
            SponsorSegment { start_ms: 10_000, end_ms: 30_000, category: 1, action: 1 },
            SponsorSegment { start_ms: 15_000, end_ms: 20_000, category: 2, action: 1 },
            SponsorSegment { start_ms: 28_000, end_ms: 35_000, category: 3, action: 2 },
        ]);

        // Different categories must remain distinct
        assert_eq!(timeline.len(), 3);
        // At 25_000, inner [15_000, 20_000) already ended, but outer [10_000, 30_000) must still match!
        assert_eq!(timeline.check_position(25_000), Some((30_000, 1, 1)));
        // At 29_000, action 1 (auto-skip) should be preferred over action 2 (notice)
        assert_eq!(timeline.check_position(29_000), Some((30_000, 1, 1)));
        // At 32_000, only category 3 action 2 remains
        assert_eq!(timeline.check_position(32_000), Some((35_000, 3, 2)));
    }
}
