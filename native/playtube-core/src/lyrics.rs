/// High-performance Native LRC & Synced Lyrics Engine for PlayTube
/// Parses LRC text in native Rust, offloading String/RegExp allocation and GC pressure from the JVM.

#[derive(Debug, Clone, PartialEq)]
pub struct LrcLine {
    pub time_ms: u64,
    pub text: String,
}

#[derive(Debug, Default, Clone)]
pub struct LrcTimeline {
    pub lines: Vec<LrcLine>,
}

impl LrcTimeline {
    pub fn new() -> Self {
        Self { lines: Vec::new() }
    }

    /// Parses LRC content with multiple timestamps and karaoke tags, reusing scratch buffers
    /// across lines to minimize intermediate heap allocations.
    /// Example: "[00:12.50]Hello world\n[00:15.00][01:20.00]Chorus <00:16.00>word"
    pub fn parse(content: &str) -> Self {
        let mut timed: Vec<LrcLine> = Vec::with_capacity(64);
        let mut stamps: Vec<u64> = Vec::with_capacity(4);
        let mut scratch = String::with_capacity(128);

        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }

            let bytes = line.as_bytes();
            let mut i = 0;
            stamps.clear();

            // Parse one or more [mm:ss.xx] timestamps at the start of the line
            while i < bytes.len() && bytes[i] == b'[' {
                let start = i + 1;
                let mut end = start;
                while end < bytes.len() && bytes[end] != b']' {
                    end += 1;
                }
                if end >= bytes.len() {
                    break;
                }

                // Try parsing timestamp mm:ss.xx
                if let Some(ms) = parse_stamp_slice(&bytes[start..end]) {
                    stamps.push(ms);
                    i = end + 1;
                } else {
                    // It's a metadata tag like [ar:Artist] or [ti:Title]
                    break;
                }
            }

            if stamps.is_empty() {
                continue;
            }

            // Remainder of line is the lyrics text. Strip inline word timestamps like <00:12.34>
            let raw_text = &line[i..];
            let trimmed = if raw_text.as_bytes().contains(&b'<') {
                strip_karaoke_tags_into(raw_text, &mut scratch);
                scratch.trim()
            } else {
                raw_text.trim()
            };
            if trimmed.is_empty() {
                continue;
            }

            for &ms in &stamps {
                timed.push(LrcLine {
                    time_ms: ms,
                    text: trimmed.to_owned(),
                });
            }
        }

        // Sort by timestamp
        timed.sort_by_key(|l| l.time_ms);

        Self { lines: timed }
    }

    /// Fast binary search: finds the active lyric line index for the current playback position.
    /// Returns -1 if the position is before the first lyric line.
    #[inline]
    pub fn find_active_index(&self, pos_ms: u64) -> i32 {
        if self.lines.is_empty() || pos_ms < self.lines[0].time_ms {
            return -1;
        }

        let mut low = 0;
        let mut high = self.lines.len() - 1;

        while low <= high {
            let mid = low + (high - low) / 2;
            if self.lines[mid].time_ms <= pos_ms {
                if mid + 1 == self.lines.len() || self.lines[mid + 1].time_ms > pos_ms {
                    return mid as i32;
                }
                low = mid + 1;
            } else {
                if mid == 0 {
                    break;
                }
                high = mid - 1;
            }
        }

        -1
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.lines.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.lines.is_empty()
    }
}

/// Parses byte slice "mm:ss.xx" or "mm:ss.xxx" into milliseconds.
fn parse_stamp_slice(slice: &[u8]) -> Option<u64> {
    // Expected format: mm:ss.xx (e.g. "00:12.50")
    let colon_pos = slice.iter().position(|&b| b == b':')?;
    let dot_pos = slice[colon_pos + 1..].iter().position(|&b| b == b'.' || b == b':').map(|p| p + colon_pos + 1)?;

    let min_str = std::str::from_utf8(&slice[..colon_pos]).ok()?;
    let sec_str = std::str::from_utf8(&slice[colon_pos + 1..dot_pos]).ok()?;
    let frac_str = std::str::from_utf8(&slice[dot_pos + 1..]).ok()?;

    let min: u64 = min_str.parse().ok()?;
    let sec: u64 = sec_str.parse().ok()?;

    let frac: u64 = frac_str.parse().ok()?;
    let frac_len = frac_str.len();

    let frac_ms = match frac_len {
        1 => frac * 100,
        2 => frac * 10,
        3 => frac,
        _ => frac / (10u64.pow((frac_len - 3) as u32)),
    };

    Some(min * 60_000 + sec * 1_000 + frac_ms)
}

/// Strips word-level timing tags like `<00:12.34>` into a reusable scratch buffer.
fn strip_karaoke_tags_into(text: &str, out: &mut String) {
    out.clear();
    out.reserve(text.len());
    let mut inside = false;

    for ch in text.chars() {
        if ch == '<' {
            inside = true;
        } else if ch == '>' {
            inside = false;
        } else if !inside {
            out.push(ch);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_lrc() {
        let lrc = r#"
[ti:Test Song]
[ar:Artist]
[00:10.00]Line 1
[00:20.50]Line 2 with <00:21.00>karaoke <00:22.00>words
[00:30.00][01:00.00]Double stamped line
"#;
        let timeline = LrcTimeline::parse(lrc);
        assert_eq!(timeline.len(), 4);
        assert_eq!(timeline.lines[0].time_ms, 10_000);
        assert_eq!(timeline.lines[0].text, "Line 1");
        assert_eq!(timeline.lines[1].time_ms, 20_500);
        assert_eq!(timeline.lines[1].text, "Line 2 with karaoke words");
        assert_eq!(timeline.lines[2].time_ms, 30_000);
        assert_eq!(timeline.lines[2].text, "Double stamped line");
        assert_eq!(timeline.lines[3].time_ms, 60_000);
        assert_eq!(timeline.lines[3].text, "Double stamped line");
    }

    #[test]
    fn test_find_active_index() {
        let lrc = "[00:10.00]First\n[00:20.00]Second\n[00:30.00]Third";
        let timeline = LrcTimeline::parse(lrc);

        assert_eq!(timeline.find_active_index(5_000), -1);
        assert_eq!(timeline.find_active_index(10_000), 0);
        assert_eq!(timeline.find_active_index(15_000), 0);
        assert_eq!(timeline.find_active_index(20_000), 1);
        assert_eq!(timeline.find_active_index(29_999), 1);
        assert_eq!(timeline.find_active_index(30_000), 2);
        assert_eq!(timeline.find_active_index(100_000), 2);
    }
}
