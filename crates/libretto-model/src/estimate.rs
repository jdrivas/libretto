// Estimate segment timings from track durations and word counts.
//
// Given a BaseLibretto and a TimingOverlay with track durations but empty
// segment_times, this module fills in estimated start times by distributing
// each track's duration proportionally across its segments' word counts.

use crate::base_libretto::{BaseLibretto, MusicalNumber, SegmentType};
use crate::timing_overlay::{SegmentTime, TimingOverlay, TrackTiming};

/// Result of an estimation pass.
#[derive(Debug)]
pub struct EstimateResult {
    /// The overlay with segment_times filled in.
    pub overlay: TimingOverlay,
    /// Per-track statistics.
    pub stats: Vec<TrackEstimateStats>,
    /// Warnings encountered during estimation.
    pub warnings: Vec<String>,
}

/// Statistics for a single track's estimation.
#[derive(Debug)]
pub struct TrackEstimateStats {
    pub track_title: String,
    pub disc_number: Option<u32>,
    pub track_number: Option<u32>,
    pub duration: f64,
    pub segments_estimated: usize,
    pub total_word_weight: f64,
}

/// Minimum weight for segments with no text (directions, interludes).
const MIN_SEGMENT_WEIGHT: f64 = 0.5;

/// Calculate word weight for a segment's text.
fn word_weight(text: &Option<String>, seg_type: &SegmentType) -> f64 {
    match seg_type {
        SegmentType::Direction | SegmentType::Interlude => MIN_SEGMENT_WEIGHT,
        _ => {
            let count = text.as_deref()
                .map(|t| t.split_whitespace().count())
                .unwrap_or(0);
            if count == 0 { MIN_SEGMENT_WEIGHT } else { count as f64 }
        }
    }
}

/// Estimate segment timings for all tracks in the overlay.
///
/// For each track that has:
/// - `duration_seconds` set
/// - `number_ids` referencing numbers in the base libretto
/// - Empty `segment_times`
///
/// The function distributes the track duration across the number's segments
/// proportionally by word count.
///
/// Multi-track numbers (e.g., a finale spanning 3 tracks) are handled by
/// pooling their total duration, distributing segments across the combined
/// span, and then splitting back into per-track offsets.
pub fn estimate_timings(base: &BaseLibretto, overlay: &TimingOverlay) -> EstimateResult {
    let mut result_overlay = overlay.clone();
    let mut stats = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    // Build a map of number_id → list of track indices that reference it.
    // Preserve track order (by index) for multi-track numbers.
    let mut number_to_tracks: std::collections::HashMap<&str, Vec<usize>> =
        std::collections::HashMap::new();
    for (i, track) in overlay.track_timings.iter().enumerate() {
        for nid in &track.number_ids {
            number_to_tracks.entry(nid.as_str()).or_default().push(i);
        }
    }

    // Track which tracks we've already estimated (avoid double-processing
    // multi-track numbers from different number_ids on the same track).
    let mut estimated_tracks: std::collections::HashSet<usize> = std::collections::HashSet::new();

    // Process each unique number_id
    for (number_id, track_indices) in &number_to_tracks {
        let number = match base.find_number(number_id) {
            Some(n) => n,
            None => {
                warnings.push(format!(
                    "Number '{}' referenced by overlay but not found in base libretto",
                    number_id
                ));
                continue;
            }
        };

        // Skip if no segments
        if number.segments.is_empty() {
            continue;
        }

        // Collect track durations; skip if any track is missing duration
        let track_durations: Vec<(usize, f64)> = track_indices.iter()
            .filter_map(|&i| {
                let track = &overlay.track_timings[i];
                // Skip tracks that already have segment_times filled in
                if !track.segment_times.is_empty() {
                    return None;
                }
                track.duration_seconds.map(|d| (i, d))
            })
            .collect();

        if track_durations.is_empty() {
            continue;
        }

        // For tracks that reference ONLY this number_id, we can do clean estimation.
        // For tracks that reference multiple number_ids, we need to split the duration.
        if track_durations.len() == 1 {
            let (track_idx, duration) = track_durations[0];
            if estimated_tracks.contains(&track_idx) {
                continue;
            }

            // How many numbers does this track contain?
            let track = &overlay.track_timings[track_idx];
            let all_segments = collect_track_segments(base, track, &mut warnings);
            let segment_times = distribute_segments(&all_segments, duration);

            let stat = TrackEstimateStats {
                track_title: track.track_title.clone(),
                disc_number: track.disc_number,
                track_number: track.track_number,
                duration,
                segments_estimated: segment_times.len(),
                total_word_weight: all_segments.iter().map(|s| s.weight).sum(),
            };
            stats.push(stat);

            result_overlay.track_timings[track_idx].segment_times = segment_times;
            estimated_tracks.insert(track_idx);
        } else {
            // Multi-track number: pool duration and distribute
            // Skip if any of these tracks are already estimated
            if track_durations.iter().any(|(i, _)| estimated_tracks.contains(i)) {
                continue;
            }

            let total_duration: f64 = track_durations.iter().map(|(_, d)| *d).sum();
            let segments = collect_number_segments(number);

            if segments.is_empty() {
                continue;
            }

            // Distribute across total duration
            let all_times = distribute_segments(&segments, total_duration);

            // Split into per-track buckets based on cumulative track durations
            let mut cumulative = 0.0;
            let mut time_iter = all_times.into_iter().peekable();

            for (track_idx, track_duration) in &track_durations {
                let track_end = cumulative + track_duration;
                let mut track_segments = Vec::new();

                while let Some(st) = time_iter.peek() {
                    if st.start < track_end || time_iter.len() == 1 {
                        let mut seg = time_iter.next().unwrap();
                        // Adjust start time to be relative to track start
                        seg.start = (seg.start - cumulative).max(0.0);
                        track_segments.push(seg);
                    } else {
                        break;
                    }
                }

                let track = &overlay.track_timings[*track_idx];
                let stat = TrackEstimateStats {
                    track_title: track.track_title.clone(),
                    disc_number: track.disc_number,
                    track_number: track.track_number,
                    duration: *track_duration,
                    segments_estimated: track_segments.len(),
                    total_word_weight: segments.iter().map(|s| s.weight).sum::<f64>() / track_durations.len() as f64,
                };
                stats.push(stat);

                result_overlay.track_timings[*track_idx].segment_times = track_segments;
                estimated_tracks.insert(*track_idx);
                cumulative = track_end;
            }
        }
    }

    EstimateResult { overlay: result_overlay, stats, warnings }
}

/// A weighted segment for distribution.
struct WeightedSegment {
    id: String,
    weight: f64,
}

/// Collect all segments for a single musical number, with word weights.
fn collect_number_segments(number: &MusicalNumber) -> Vec<WeightedSegment> {
    number.segments.iter()
        .map(|s| WeightedSegment {
            id: s.id.clone(),
            weight: word_weight(&s.text, &s.segment_type),
        })
        .collect()
}

/// Collect all segments for a track (which may reference multiple numbers).
fn collect_track_segments(
    base: &BaseLibretto,
    track: &TrackTiming,
    warnings: &mut Vec<String>,
) -> Vec<WeightedSegment> {
    let mut segments = Vec::new();
    for nid in &track.number_ids {
        match base.find_number(nid) {
            Some(number) => {
                segments.extend(collect_number_segments(number));
            }
            None => {
                warnings.push(format!(
                    "Track '{}': number '{}' not found in base libretto",
                    track.track_title, nid
                ));
            }
        }
    }
    segments
}

/// Distribute weighted segments across a duration, returning estimated start times.
fn distribute_segments(segments: &[WeightedSegment], duration: f64) -> Vec<SegmentTime> {
    if segments.is_empty() || duration <= 0.0 {
        return Vec::new();
    }

    let total_weight: f64 = segments.iter().map(|s| s.weight).sum();
    if total_weight == 0.0 {
        return Vec::new();
    }

    let mut result = Vec::with_capacity(segments.len());
    let mut cumulative = 0.0;

    for seg in segments {
        let start = (cumulative / total_weight) * duration;
        result.push(SegmentTime {
            segment_id: seg.id.clone(),
            start: round_to_ms(start),
        });
        cumulative += seg.weight;
    }

    result
}

/// Round to millisecond precision.
fn round_to_ms(seconds: f64) -> f64 {
    (seconds * 1000.0).round() / 1000.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base_libretto::*;
    use crate::timing_overlay::*;

    fn test_base() -> BaseLibretto {
        let mut lib = BaseLibretto::new(OperaMetadata {
            title: "Test Opera".to_string(),
            composer: "Test".to_string(),
            librettist: None,
            language: "it".to_string(),
            translation_language: None,
            year: None,
        });
        lib.numbers.push(MusicalNumber {
            id: "no-1".to_string(),
            label: "No. 1".to_string(),
            number_type: NumberType::Aria,
            act: "1".to_string(),
            scene: None,
            segments: vec![
                Segment {
                    id: "no-1-001".to_string(),
                    segment_type: SegmentType::Sung,
                    character: Some("A".to_string()),
                    text: Some("one two three".to_string()), // 3 words
                    translation: None,
                    direction: None,
                },
                Segment {
                    id: "no-1-002".to_string(),
                    segment_type: SegmentType::Sung,
                    character: Some("B".to_string()),
                    text: Some("four five six seven eight nine ten eleven twelve".to_string()), // 9 words
                    translation: None,
                    direction: None,
                },
                Segment {
                    id: "no-1-003".to_string(),
                    segment_type: SegmentType::Direction,
                    character: None,
                    text: None,
                    translation: None,
                    direction: Some("exits".to_string()),
                },
            ],
        });
        lib
    }

    fn test_overlay(duration: f64) -> TimingOverlay {
        TimingOverlay {
            version: "1.0".to_string(),
            base_libretto: "test".to_string(),
            recording: RecordingMetadata {
                conductor: None, orchestra: None, year: None, label: None, album_title: None,
            },
            contributors: vec![],
            omitted_numbers: vec![],
            track_timings: vec![TrackTiming {
                track_title: "Track 1".to_string(),
                disc_number: Some(1),
                track_number: Some(1),
                duration_seconds: Some(duration),
                number_ids: vec!["no-1".to_string()],
                segment_times: vec![],
            }],
        }
    }

    #[test]
    fn test_estimate_basic() {
        let base = test_base();
        let overlay = test_overlay(125.0); // 125 seconds

        let result = estimate_timings(&base, &overlay);
        assert!(result.warnings.is_empty(), "warnings: {:?}", result.warnings);

        let times = &result.overlay.track_timings[0].segment_times;
        assert_eq!(times.len(), 3);

        // Weights: 3, 9, 0.5 = 12.5 total
        // Seg 1: start = 0.0
        assert_eq!(times[0].segment_id, "no-1-001");
        assert_eq!(times[0].start, 0.0);

        // Seg 2: start = (3/12.5) * 125 = 30.0
        assert_eq!(times[1].segment_id, "no-1-002");
        assert_eq!(times[1].start, 30.0);

        // Seg 3: start = (12/12.5) * 125 = 120.0
        assert_eq!(times[2].segment_id, "no-1-003");
        assert_eq!(times[2].start, 120.0);
    }

    #[test]
    fn test_estimate_skips_existing_times() {
        let base = test_base();
        let mut overlay = test_overlay(125.0);
        // Pre-fill segment_times — should be left alone
        overlay.track_timings[0].segment_times = vec![
            SegmentTime { segment_id: "no-1-001".to_string(), start: 0.0 },
        ];

        let result = estimate_timings(&base, &overlay);
        // Should still have the original single segment_time
        assert_eq!(result.overlay.track_timings[0].segment_times.len(), 1);
    }

    #[test]
    fn test_estimate_no_duration() {
        let base = test_base();
        let mut overlay = test_overlay(100.0);
        overlay.track_timings[0].duration_seconds = None;

        let result = estimate_timings(&base, &overlay);
        assert!(result.overlay.track_timings[0].segment_times.is_empty());
    }

    #[test]
    fn test_estimate_multi_track_number() {
        let mut base = test_base();
        // Add a number with 4 segments
        base.numbers.push(MusicalNumber {
            id: "no-2".to_string(),
            label: "No. 2".to_string(),
            number_type: NumberType::Finale,
            act: "1".to_string(),
            scene: None,
            segments: vec![
                Segment {
                    id: "no-2-001".to_string(),
                    segment_type: SegmentType::Sung,
                    character: Some("A".to_string()),
                    text: Some("one two three four five".to_string()), // 5 words
                    translation: None,
                    direction: None,
                },
                Segment {
                    id: "no-2-002".to_string(),
                    segment_type: SegmentType::Sung,
                    character: Some("B".to_string()),
                    text: Some("six seven eight nine ten".to_string()), // 5 words
                    translation: None,
                    direction: None,
                },
                Segment {
                    id: "no-2-003".to_string(),
                    segment_type: SegmentType::Sung,
                    character: Some("A".to_string()),
                    text: Some("eleven twelve thirteen fourteen fifteen".to_string()), // 5
                    translation: None,
                    direction: None,
                },
                Segment {
                    id: "no-2-004".to_string(),
                    segment_type: SegmentType::Sung,
                    character: Some("B".to_string()),
                    text: Some("sixteen seventeen eighteen nineteen twenty".to_string()), // 5
                    translation: None,
                    direction: None,
                },
            ],
        });

        let overlay = TimingOverlay {
            version: "1.0".to_string(),
            base_libretto: "test".to_string(),
            recording: RecordingMetadata {
                conductor: None, orchestra: None, year: None, label: None, album_title: None,
            },
            contributors: vec![],
            omitted_numbers: vec![],
            track_timings: vec![
                TrackTiming {
                    track_title: "Finale Part 1".to_string(),
                    disc_number: Some(1),
                    track_number: Some(1),
                    duration_seconds: Some(50.0), // half the time
                    number_ids: vec!["no-2".to_string()],
                    segment_times: vec![],
                },
                TrackTiming {
                    track_title: "Finale Part 2".to_string(),
                    disc_number: Some(1),
                    track_number: Some(2),
                    duration_seconds: Some(50.0), // half the time
                    number_ids: vec!["no-2".to_string()],
                    segment_times: vec![],
                },
            ],
        };

        let result = estimate_timings(&base, &overlay);
        assert!(result.warnings.is_empty(), "warnings: {:?}", result.warnings);

        // 4 segments, equal weight, 100s total → each ~25s
        // Track 1 (50s): should get seg 1 (0s) and seg 2 (25s)
        // Track 2 (50s): should get seg 3 (0s) and seg 4 (25s)
        let t1 = &result.overlay.track_timings[0].segment_times;
        let t2 = &result.overlay.track_timings[1].segment_times;
        assert_eq!(t1.len(), 2, "Track 1 segments: {:?}", t1);
        assert_eq!(t2.len(), 2, "Track 2 segments: {:?}", t2);
        assert_eq!(t1[0].segment_id, "no-2-001");
        assert_eq!(t1[1].segment_id, "no-2-002");
        assert_eq!(t2[0].segment_id, "no-2-003");
        assert_eq!(t2[1].segment_id, "no-2-004");

        // Start times should be relative to each track
        assert_eq!(t1[0].start, 0.0);
        assert_eq!(t2[0].start, 0.0);
    }
}
