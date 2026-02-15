use serde::{Deserialize, Serialize};

/// A timing overlay: recording-specific timing data that references
/// a base libretto's segment IDs.
///
/// This is the output of the timing tool — it maps segment IDs to
/// start times within specific audio tracks for a particular recording.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimingOverlay {
    pub version: String,
    /// Path to the base libretto this overlay references (relative to library root).
    pub base_libretto: String,
    pub recording: RecordingMetadata,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contributors: Vec<Contributor>,
    pub track_timings: Vec<TrackTiming>,
    /// Numbers from the base libretto that this recording does not perform.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub omitted_numbers: Vec<OmittedNumber>,
}

/// Metadata about the specific recording this timing is for.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conductor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orchestra: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub year: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album_title: Option<String>,
}

/// A person who contributed timing data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Contributor {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
}

/// Timing data for a single audio track.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrackTiming {
    /// Track title as it appears in the album metadata.
    pub track_title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disc_number: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track_number: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<f64>,
    /// Which musical number IDs from the base libretto this track contains.
    pub number_ids: Vec<String>,
    /// Timed segment references, ordered by start time.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub segment_times: Vec<SegmentTime>,
}

/// A musical number explicitly declared as not performed in this recording.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OmittedNumber {
    /// The number ID from the base libretto (e.g., "no-24-aria").
    pub number_id: String,
    /// Human-readable reason for the omission.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
}

/// A single segment's timing within a track.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegmentTime {
    /// References a segment ID in the base libretto.
    pub segment_id: String,
    /// Start time in seconds from the beginning of the track.
    pub start: f64,
}

impl TimingOverlay {
    /// Get all segment IDs referenced in this overlay, in order.
    pub fn segment_ids(&self) -> Vec<&str> {
        self.track_timings
            .iter()
            .flat_map(|t| t.segment_times.iter().map(|s| s.segment_id.as_str()))
            .collect()
    }

    /// Get all number IDs referenced across all tracks.
    pub fn covered_number_ids(&self) -> Vec<&str> {
        let mut ids: Vec<&str> = self.track_timings
            .iter()
            .flat_map(|t| t.number_ids.iter().map(|s| s.as_str()))
            .collect();
        ids.sort();
        ids.dedup();
        ids
    }

    /// Get all explicitly omitted number IDs.
    pub fn omitted_number_ids(&self) -> Vec<&str> {
        self.omitted_numbers.iter().map(|o| o.number_id.as_str()).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_overlay() -> TimingOverlay {
        TimingOverlay {
            version: "1.0".to_string(),
            base_libretto: "mozart/le-nozze-di-figaro/base.libretto.json".to_string(),
            recording: RecordingMetadata {
                conductor: Some("Carlo Maria Giulini".to_string()),
                orchestra: Some("Philharmonia Orchestra".to_string()),
                year: Some(1959),
                label: Some("EMI".to_string()),
                album_title: Some("Le nozze di Figaro (Giulini)".to_string()),
            },
            contributors: vec![Contributor {
                name: "Test User".to_string(),
                role: Some("timing".to_string()),
                date: Some("2026-02-14".to_string()),
            }],
            track_timings: vec![TrackTiming {
                track_title: "Cinque... dieci... venti...".to_string(),
                disc_number: Some(1),
                track_number: Some(2),
                duration_seconds: Some(195.0),
                number_ids: vec!["no-1-duettino".to_string()],
                segment_times: vec![
                    SegmentTime {
                        segment_id: "no-1-001".to_string(),
                        start: 0.0,
                    },
                    SegmentTime {
                        segment_id: "no-1-002".to_string(),
                        start: 12.5,
                    },
                ],
            }],
            omitted_numbers: vec![OmittedNumber {
                number_id: "no-24-aria".to_string(),
                reason: Some("Traditional cut".to_string()),
            }],
        }
    }

    #[test]
    fn test_segment_ids() {
        let overlay = sample_overlay();
        let ids = overlay.segment_ids();
        assert_eq!(ids, vec!["no-1-001", "no-1-002"]);
    }

    #[test]
    fn test_json_roundtrip() {
        let overlay = sample_overlay();
        let json = serde_json::to_string_pretty(&overlay).unwrap();
        let parsed: TimingOverlay = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.recording.conductor.as_deref(), Some("Carlo Maria Giulini"));
        assert_eq!(parsed.track_timings[0].segment_times.len(), 2);
    }
}
