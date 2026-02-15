// Merge a BaseLibretto + TimingOverlay into an InterchangeLibretto.
//
// The merge resolves segment IDs from the overlay against the base libretto,
// producing a self-contained timed document ready for display systems.

use std::collections::HashMap;

use crate::base_libretto::{BaseLibretto, Segment};
use crate::interchange::{InterchangeLibretto, InterchangeOpera, InterchangeSegment, InterchangeTrack};
use crate::timing_overlay::{TimingOverlay, TrackTiming};

/// Merge a base libretto with a timing overlay to produce an interchange libretto.
///
/// Each track in the overlay becomes an `InterchangeTrack`. Segment IDs from
/// the overlay are resolved against the base libretto to pull in text,
/// translation, character, and direction fields.
///
/// Segments referenced in the overlay but missing from the base libretto
/// are included with a warning (empty text fields). Segments in the base
/// libretto but not referenced in the overlay are silently skipped.
pub fn merge(base: &BaseLibretto, overlay: &TimingOverlay) -> MergeResult {
    let mut warnings: Vec<String> = Vec::new();

    // Index all base libretto segments by ID for O(1) lookup
    let segment_map: HashMap<&str, &Segment> = base.numbers.iter()
        .flat_map(|n| n.segments.iter())
        .map(|s| (s.id.as_str(), s))
        .collect();

    // Also index number metadata by segment ID → (act, scene)
    let mut segment_context: HashMap<&str, (&str, Option<&str>)> = HashMap::new();
    for number in &base.numbers {
        for seg in &number.segments {
            segment_context.insert(seg.id.as_str(), (number.act.as_str(), number.scene.as_deref()));
        }
    }

    let opera = InterchangeOpera {
        title: base.opera.title.clone(),
        composer: base.opera.composer.clone(),
        librettist: base.opera.librettist.clone(),
        language: base.opera.language.clone(),
        translation_language: base.opera.translation_language.clone(),
        year: base.opera.year,
    };

    let tracks: Vec<InterchangeTrack> = overlay.track_timings.iter()
        .enumerate()
        .map(|(i, track)| merge_track(track, i, &segment_map, &segment_context, &overlay.recording, &mut warnings))
        .collect();

    let total_segments: usize = tracks.iter().map(|t| t.segments.len()).sum();
    let total_base_segments: usize = base.numbers.iter().map(|n| n.segments.len()).sum();
    let referenced_ids: usize = overlay.track_timings.iter()
        .map(|t| t.segment_times.len())
        .sum();

    MergeResult {
        libretto: InterchangeLibretto {
            version: "1.0".to_string(),
            opera,
            tracks,
        },
        stats: MergeStats {
            base_segments: total_base_segments,
            overlay_references: referenced_ids,
            merged_segments: total_segments,
            tracks: overlay.track_timings.len(),
        },
        warnings,
    }
}

fn merge_track(
    track: &TrackTiming,
    index: usize,
    segment_map: &HashMap<&str, &Segment>,
    segment_context: &HashMap<&str, (&str, Option<&str>)>,
    recording: &crate::timing_overlay::RecordingMetadata,
    warnings: &mut Vec<String>,
) -> InterchangeTrack {
    let segments: Vec<InterchangeSegment> = track.segment_times.iter()
        .enumerate()
        .map(|(j, st)| {
            let base_seg = segment_map.get(st.segment_id.as_str());
            if base_seg.is_none() {
                warnings.push(format!(
                    "Track '{}': segment '{}' not found in base libretto",
                    track.track_title, st.segment_id
                ));
            }

            let ctx = segment_context.get(st.segment_id.as_str());

            // Compute end time: next segment's start, or track duration
            let end = if j + 1 < track.segment_times.len() {
                Some(track.segment_times[j + 1].start)
            } else {
                track.duration_seconds
            };

            InterchangeSegment {
                start: st.start,
                end,
                segment_type: base_seg
                    .map(|s| format!("{:?}", s.segment_type).to_lowercase())
                    .unwrap_or_else(|| "sung".to_string()),
                character: base_seg.and_then(|s| s.character.clone()),
                text: base_seg.and_then(|s| s.text.clone()),
                translation: base_seg.and_then(|s| s.translation.clone()),
                direction: base_seg.and_then(|s| s.direction.clone()),
                act: ctx.map(|(act, _)| act.to_string()),
                scene: ctx.and_then(|(_, scene)| scene.map(|s| s.to_string())),
            }
        })
        .collect();

    // Derive act from the first segment's context, if available
    let act = segments.first().and_then(|s| s.act.clone());

    // Build track ID from disc/track number or index
    let track_id = match (track.disc_number, track.track_number) {
        (Some(d), Some(t)) => format!("d{d}-t{t}"),
        (None, Some(t)) => format!("t{t}"),
        _ => format!("track-{}", index + 1),
    };

    // Artist from recording metadata
    let artist = recording.conductor.as_ref().map(|c| {
        if let Some(orch) = &recording.orchestra {
            format!("{c} / {orch}")
        } else {
            c.clone()
        }
    });

    InterchangeTrack {
        track_id,
        title: track.track_title.clone(),
        album: recording.album_title.clone(),
        artist,
        disc_number: track.disc_number,
        track_number: track.track_number,
        duration_seconds: track.duration_seconds,
        act,
        scene: None,
        segments,
    }
}

/// Result of a merge operation.
pub struct MergeResult {
    pub libretto: InterchangeLibretto,
    pub stats: MergeStats,
    pub warnings: Vec<String>,
}

/// Statistics about the merge.
pub struct MergeStats {
    pub base_segments: usize,
    pub overlay_references: usize,
    pub merged_segments: usize,
    pub tracks: usize,
}

/// Generate a scaffold TimingOverlay from a BaseLibretto.
///
/// Creates one TrackTiming per musical number, with all segment IDs
/// listed but start times set to 0.0. This gives a template to fill
/// in with actual timing data.
pub fn scaffold_overlay(base: &BaseLibretto, base_path: &str) -> TimingOverlay {
    let track_timings: Vec<TrackTiming> = base.numbers.iter()
        .map(|number| {
            let segment_times: Vec<crate::timing_overlay::SegmentTime> = number.segments.iter()
                .map(|seg| crate::timing_overlay::SegmentTime {
                    segment_id: seg.id.clone(),
                    start: 0.0,
                })
                .collect();

            TrackTiming {
                track_title: number.label.clone(),
                disc_number: None,
                track_number: None,
                duration_seconds: None,
                number_ids: vec![number.id.clone()],
                start_segment_id: None,
                segment_times,
            }
        })
        .collect();

    TimingOverlay {
        version: "1.0".to_string(),
        base_libretto: base_path.to_string(),
        recording: crate::timing_overlay::RecordingMetadata {
            conductor: None,
            orchestra: None,
            year: None,
            label: None,
            album_title: None,
        },
        contributors: Vec::new(),
        track_timings,
        omitted_numbers: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base_libretto::*;
    use crate::timing_overlay::*;

    fn sample_base() -> BaseLibretto {
        let mut libretto = BaseLibretto::new(OperaMetadata {
            title: "Le nozze di Figaro".to_string(),
            composer: "Mozart".to_string(),
            librettist: Some("Da Ponte".to_string()),
            language: "it".to_string(),
            translation_language: Some("en".to_string()),
            year: Some(1786),
        });
        libretto.numbers.push(MusicalNumber {
            id: "no-1-duettino".to_string(),
            label: "N° 1: Duettino".to_string(),
            number_type: NumberType::Duettino,
            act: "1".to_string(),
            scene: Some("1".to_string()),
            segments: vec![
                Segment {
                    id: "no-1-duettino-001".to_string(),
                    segment_type: SegmentType::Sung,
                    character: Some("FIGARO".to_string()),
                    text: Some("Cinque... dieci...".to_string()),
                    translation: Some("Five... ten...".to_string()),
                    direction: None,
                },
                Segment {
                    id: "no-1-duettino-002".to_string(),
                    segment_type: SegmentType::Sung,
                    character: Some("SUSANNA".to_string()),
                    text: Some("Ora sì ch'io son contenta.".to_string()),
                    translation: Some("How happy I am now.".to_string()),
                    direction: None,
                },
            ],
        });
        libretto
    }

    fn sample_overlay() -> TimingOverlay {
        TimingOverlay {
            version: "1.0".to_string(),
            base_libretto: "base.libretto.json".to_string(),
            recording: RecordingMetadata {
                conductor: Some("Giulini".to_string()),
                orchestra: Some("Philharmonia".to_string()),
                year: Some(1959),
                label: Some("EMI".to_string()),
                album_title: Some("Le nozze di Figaro".to_string()),
            },
            contributors: vec![],
            omitted_numbers: vec![],
            track_timings: vec![TrackTiming {
                track_title: "Cinque... dieci...".to_string(),
                disc_number: Some(1),
                track_number: Some(2),
                duration_seconds: Some(195.0),
                number_ids: vec!["no-1-duettino".to_string()],
                start_segment_id: None,
                segment_times: vec![
                    SegmentTime { segment_id: "no-1-duettino-001".to_string(), start: 0.0 },
                    SegmentTime { segment_id: "no-1-duettino-002".to_string(), start: 12.5 },
                ],
            }],
        }
    }

    #[test]
    fn test_merge() {
        let base = sample_base();
        let overlay = sample_overlay();
        let result = merge(&base, &overlay);

        assert!(result.warnings.is_empty());
        assert_eq!(result.libretto.tracks.len(), 1);

        let track = &result.libretto.tracks[0];
        assert_eq!(track.track_id, "d1-t2");
        assert_eq!(track.title, "Cinque... dieci...");
        assert_eq!(track.album.as_deref(), Some("Le nozze di Figaro"));
        assert_eq!(track.artist.as_deref(), Some("Giulini / Philharmonia"));
        assert_eq!(track.segments.len(), 2);

        let seg0 = &track.segments[0];
        assert_eq!(seg0.start, 0.0);
        assert_eq!(seg0.end, Some(12.5)); // computed from next segment
        assert_eq!(seg0.character.as_deref(), Some("FIGARO"));
        assert_eq!(seg0.text.as_deref(), Some("Cinque... dieci..."));
        assert_eq!(seg0.translation.as_deref(), Some("Five... ten..."));
        assert_eq!(seg0.act.as_deref(), Some("1"));
        assert_eq!(seg0.scene.as_deref(), Some("1"));

        let seg1 = &track.segments[1];
        assert_eq!(seg1.start, 12.5);
        assert_eq!(seg1.end, Some(195.0)); // track duration
        assert_eq!(seg1.character.as_deref(), Some("SUSANNA"));
    }

    #[test]
    fn test_merge_unknown_segment() {
        let base = sample_base();
        let mut overlay = sample_overlay();
        overlay.track_timings[0].segment_times.push(
            SegmentTime { segment_id: "no-1-duettino-999".to_string(), start: 50.0 }
        );

        let result = merge(&base, &overlay);
        assert_eq!(result.warnings.len(), 1);
        assert!(result.warnings[0].contains("no-1-duettino-999"));
    }

    #[test]
    fn test_scaffold_overlay() {
        let base = sample_base();
        let overlay = scaffold_overlay(&base, "base.libretto.json");

        assert_eq!(overlay.track_timings.len(), 1);
        assert_eq!(overlay.track_timings[0].track_title, "N° 1: Duettino");
        assert_eq!(overlay.track_timings[0].segment_times.len(), 2);
        assert_eq!(overlay.track_timings[0].segment_times[0].segment_id, "no-1-duettino-001");
        assert_eq!(overlay.track_timings[0].segment_times[0].start, 0.0);
    }

    #[test]
    fn test_merge_stats() {
        let base = sample_base();
        let overlay = sample_overlay();
        let result = merge(&base, &overlay);

        assert_eq!(result.stats.base_segments, 2);
        assert_eq!(result.stats.overlay_references, 2);
        assert_eq!(result.stats.merged_segments, 2);
        assert_eq!(result.stats.tracks, 1);
    }
}
