// Estimate segment timings from track durations and word counts.
//
// Given a BaseLibretto and a TimingOverlay with track durations but empty
// segment_times, this module fills in estimated start times by distributing
// each track's duration proportionally across its segments' word counts.

use std::collections::HashMap;

use crate::base_libretto::{BaseLibretto, MusicalNumber, SegmentType};
use crate::resolve;
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

/// Recitative segments are spoken-sung at roughly 2× the pace of sung text,
/// so their word weight is discounted by this factor.
const RECITATIVE_DISCOUNT: f64 = 0.5;

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
/// If tracks have `start_segment_id` set (from anchor resolution), uses
/// those boundaries to precisely partition segments across tracks.
/// Otherwise, falls back to number-based assignment using `number_ids`.
pub fn estimate_timings(base: &BaseLibretto, overlay: &TimingOverlay) -> EstimateResult {
    let has_boundaries = overlay.track_timings.iter()
        .any(|t| t.start_segment_id.is_some());

    if has_boundaries {
        estimate_with_boundaries(base, overlay)
    } else {
        estimate_by_numbers(base, overlay)
    }
}

/// Boundary-based estimation: uses `start_segment_id` to determine which
/// segments belong to each track, regardless of number boundaries.
///
/// Builds a global ordered segment list from all numbers covered by the
/// overlay, then partitions it using the start_segment_id markers.
fn estimate_with_boundaries(base: &BaseLibretto, overlay: &TimingOverlay) -> EstimateResult {
    let mut result_overlay = overlay.clone();
    let mut stats = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    // Build global ordered segment list from all covered numbers (in libretto order)
    let covered: Vec<&str> = overlay.covered_number_ids();
    let all_segments: Vec<WeightedSegment> = base.numbers.iter()
        .filter(|n| covered.contains(&n.id.as_str()))
        .flat_map(|n| collect_number_segments(n))
        .collect();

    // Build segment_id → position index
    let seg_index: HashMap<&str, usize> = all_segments.iter()
        .enumerate()
        .map(|(i, s)| (s.id.as_str(), i))
        .collect();

    // Build resolve infrastructure once for recitative classification
    let resolve_candidates = resolve::build_segment_index(base);
    let all_nids: Vec<String> = covered.iter().map(|s| s.to_string()).collect();

    for (i, track) in overlay.track_timings.iter().enumerate() {
        // Skip tracks that already have segment_times
        if !track.segment_times.is_empty() {
            continue;
        }
        let duration = match track.duration_seconds {
            Some(d) => d,
            None => continue,
        };

        // Find start position from start_segment_id or first segment of first number
        let start_pos = match &track.start_segment_id {
            Some(sid) => match seg_index.get(sid.as_str()) {
                Some(&pos) => pos,
                None => {
                    warnings.push(format!(
                        "D{}T{} '{}': start_segment_id '{}' not found in segment index",
                        track.disc_number.unwrap_or(0),
                        track.track_number.unwrap_or(0),
                        track.track_title, sid,
                    ));
                    continue;
                }
            },
            None => {
                // Fallback: first segment of first referenced number
                match track.number_ids.first()
                    .and_then(|nid| base.find_number(nid))
                    .and_then(|n| n.segments.first())
                    .and_then(|s| seg_index.get(s.id.as_str()))
                    .copied()
                {
                    Some(pos) => pos,
                    None => continue,
                }
            }
        };

        // Find end position: the next track's start_segment_id boundary
        let end_pos = (i + 1..overlay.track_timings.len())
            .find_map(|j| {
                overlay.track_timings[j].start_segment_id.as_ref()
                    .and_then(|sid| seg_index.get(sid.as_str()))
                    .copied()
            })
            .unwrap_or(all_segments.len());

        if start_pos >= end_pos {
            warnings.push(format!(
                "D{}T{} '{}': empty segment range (start={}, end={})",
                track.disc_number.unwrap_or(0),
                track.track_number.unwrap_or(0),
                track.track_title, start_pos, end_pos,
            ));
            continue;
        }

        // Classify title sections and resolve sub-boundaries for recitative discount
        let section_marks = resolve_section_marks(
            &track.track_title, start_pos, end_pos,
            &seg_index, &resolve_candidates, &all_nids,
        );

        // Build adjusted weights: recitative segments get discounted
        let track_segments: Vec<WeightedSegment> = all_segments[start_pos..end_pos]
            .iter()
            .enumerate()
            .map(|(j, seg)| {
                let global_pos = start_pos + j;
                let is_recit = section_marks.iter()
                    .rev()
                    .find(|(pos, _)| *pos <= global_pos)
                    .map(|(_, recit)| *recit)
                    .unwrap_or(false);
                WeightedSegment {
                    id: seg.id.clone(),
                    weight: if is_recit { seg.weight * RECITATIVE_DISCOUNT } else { seg.weight },
                }
            })
            .collect();

        let segment_times = distribute_segments(&track_segments, duration);

        let stat = TrackEstimateStats {
            track_title: track.track_title.clone(),
            disc_number: track.disc_number,
            track_number: track.track_number,
            duration,
            segments_estimated: segment_times.len(),
            total_word_weight: track_segments.iter().map(|s| s.weight).sum(),
        };
        stats.push(stat);
        result_overlay.track_timings[i].segment_times = segment_times;
    }

    EstimateResult { overlay: result_overlay, stats, warnings }
}

/// Resolve title section anchors to global segment positions, returning
/// (position, is_recitative) pairs sorted by position.
fn resolve_section_marks(
    title: &str,
    start_pos: usize,
    end_pos: usize,
    seg_index: &HashMap<&str, usize>,
    candidates: &[resolve::SegCandidate<'_>],
    all_nids: &[String],
) -> Vec<(usize, bool)> {
    let title_anchors = resolve::classify_title_anchors(title);
    let mut marks: Vec<(usize, bool)> = Vec::new();

    for ta in &title_anchors {
        if let Some((seg_id, _)) = resolve::match_anchor(&ta.anchor, all_nids, candidates) {
            if let Some(&pos) = seg_index.get(seg_id.as_str()) {
                if pos >= start_pos && pos < end_pos {
                    marks.push((pos, ta.is_recitative));
                }
            }
        }
    }

    marks.sort_by_key(|(pos, _)| *pos);
    marks
}

/// Number-based estimation (legacy): uses `number_ids` to assign segments
/// to tracks. Multi-track numbers are handled by pooling duration.
fn estimate_by_numbers(base: &BaseLibretto, overlay: &TimingOverlay) -> EstimateResult {
    let mut result_overlay = overlay.clone();
    let mut stats = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    // Build a map of number_id → list of track indices that reference it.
    let mut number_to_tracks: HashMap<&str, Vec<usize>> = HashMap::new();
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

        if track_durations.len() == 1 {
            let (track_idx, duration) = track_durations[0];
            if estimated_tracks.contains(&track_idx) {
                continue;
            }

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
            if track_durations.iter().any(|(i, _)| estimated_tracks.contains(i)) {
                continue;
            }

            let total_duration: f64 = track_durations.iter().map(|(_, d)| *d).sum();
            let segments = collect_number_segments(number);

            if segments.is_empty() {
                continue;
            }

            let all_times = distribute_segments(&segments, total_duration);

            let mut cumulative = 0.0;
            let mut time_iter = all_times.into_iter().peekable();

            for (track_idx, track_duration) in &track_durations {
                let track_end = cumulative + track_duration;
                let mut track_segments = Vec::new();

                while let Some(st) = time_iter.peek() {
                    if st.start < track_end || time_iter.len() == 1 {
                        let mut seg = time_iter.next().unwrap();
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
                start_segment_id: None,
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
                    start_segment_id: None,
                    segment_times: vec![],
                },
                TrackTiming {
                    track_title: "Finale Part 2".to_string(),
                    disc_number: Some(1),
                    track_number: Some(2),
                    duration_seconds: Some(50.0), // half the time
                    number_ids: vec!["no-2".to_string()],
                    start_segment_id: None,
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

    #[test]
    fn test_estimate_with_boundaries_crossover() {
        // Simulates the real-world case: number-1 has 3 segments, but the
        // 3rd segment ("Bravo, signor padrone") actually starts in track 2.
        // number-2 has 1 segment that also belongs to track 2.
        let mut base = test_base(); // no-1: 3 segments (001, 002, 003)
        base.numbers.push(MusicalNumber {
            id: "no-2".to_string(),
            label: "No. 2".to_string(),
            number_type: NumberType::Cavatina,
            act: "1".to_string(),
            scene: None,
            segments: vec![
                Segment {
                    id: "no-2-001".to_string(),
                    segment_type: SegmentType::Sung,
                    character: Some("A".to_string()),
                    text: Some("alpha beta gamma delta".to_string()), // 4 words
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
                    track_title: "Track 1".to_string(),
                    disc_number: Some(1),
                    track_number: Some(1),
                    duration_seconds: Some(100.0),
                    number_ids: vec!["no-1".to_string()],
                    // Track 1 starts at seg 001
                    start_segment_id: Some("no-1-001".to_string()),
                    segment_times: vec![],
                },
                TrackTiming {
                    track_title: "Track 2".to_string(),
                    disc_number: Some(1),
                    track_number: Some(2),
                    duration_seconds: Some(100.0),
                    number_ids: vec!["no-2".to_string()],
                    // Track 2 starts at seg 003 (crossover from no-1!)
                    start_segment_id: Some("no-1-003".to_string()),
                    segment_times: vec![],
                },
            ],
        };

        let result = estimate_timings(&base, &overlay);
        assert!(result.warnings.is_empty(), "warnings: {:?}", result.warnings);

        let t1 = &result.overlay.track_timings[0].segment_times;
        let t2 = &result.overlay.track_timings[1].segment_times;

        // Track 1: no-1-001, no-1-002 (boundary stops before no-1-003)
        assert_eq!(t1.len(), 2, "Track 1 segments: {:?}", t1);
        assert_eq!(t1[0].segment_id, "no-1-001");
        assert_eq!(t1[1].segment_id, "no-1-002");

        // Track 2: no-1-003 (crossover!) + no-2-001
        assert_eq!(t2.len(), 2, "Track 2 segments: {:?}", t2);
        assert_eq!(t2[0].segment_id, "no-1-003");
        assert_eq!(t2[1].segment_id, "no-2-001");

        // Start times relative to each track
        assert_eq!(t1[0].start, 0.0);
        assert_eq!(t2[0].start, 0.0);
    }
}
