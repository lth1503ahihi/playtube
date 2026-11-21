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
}

impl SponsorTimeline {
    pub fn new() -> Self {
        Self { segments: Vec::new() }
    }

    /// Sets and normalizes sponsor segments: sorts by start time and merges overlapping segments.
    pub fn set_segments(&mut self, mut list: Vec<SponsorSegment>) {
        if list.is_empty() {
            self.segments.clear();
            return;
        }

        // Sort by start_ms
        list.sort_by_key(|s| s.start_ms);

        let mut merged: Vec<SponsorSegment> = Vec::with_capacity(list.len());
        for seg in list {
            if seg.start_ms >= seg.end_ms {
                continue; // invalid segment
            }

            if let Some(last) = merged.last_mut() {
                // If segments overlap or touch and have same category/action, merge them
                if seg.start_ms <= last.end_ms && seg.action == last.action {
                    last.end_ms = last.end_ms.max(seg.end_ms);
                    continue;
                }
            }
            merged.push(seg);
        }

        self.segments = merged;
    }

    /// Binary search interval check: determines if pos_ms falls inside a segment.
    /// Returns Some((target_skip_pos_ms, category, action)) if inside a segment, or None otherwise.
    #[inline]
    pub fn check_position(&self, pos_ms: u64) -> Option<(u64, u8, u8)> {
        if self.segments.is_empty() {
            return None;
        }

        let mut low = 0;
        let mut high = self.segments.len() - 1;

        while low <= high {
            let mid = low + (high - low) / 2;
            let seg = &self.segments[mid];

            if pos_ms >= seg.start_ms && pos_ms < seg.end_ms {
                return Some((seg.end_ms, seg.category, seg.action));
            } else if pos_ms < seg.start_ms {
                if mid == 0 {
                    break;
                }
                high = mid - 1;
            } else {
                low = mid + 1;
            }
        }

        None
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
}
