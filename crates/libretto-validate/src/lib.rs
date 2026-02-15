use anyhow::Result;
use libretto_model::{BaseLibretto, TimingOverlay};
use std::collections::HashSet;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ValidationError {
    #[error("missing required field: {0}")]
    MissingField(String),

    #[error("duplicate segment ID: {0}")]
    DuplicateSegmentId(String),

    #[error("timing overlay references unknown segment ID: {0}")]
    UnknownSegmentId(String),

    #[error("segments not ordered by start time in track '{0}'")]
    SegmentsUnordered(String),

    #[error("segment time {0}s is negative")]
    NegativeTime(f64),

    #[error("number '{0}' is neither covered by any track nor declared as omitted")]
    UnaccountedNumber(String),

    #[error("omitted number '{0}' does not exist in the base libretto")]
    UnknownOmittedNumber(String),

    #[error("number '{0}' is both covered by a track and declared as omitted")]
    ConflictingCoverage(String),

    #[error("{0}")]
    Other(String),
}

/// Validate a base libretto or timing overlay file.
///
/// If `base_path` is provided, the file is treated as a timing overlay
/// and segment ID references are checked against the base libretto.
pub fn validate(file_path: &str, base_path: Option<&str>) -> Result<()> {
    let contents = std::fs::read_to_string(file_path)?;

    if let Some(base) = base_path {
        // Validate as timing overlay
        let overlay: TimingOverlay = serde_json::from_str(&contents)?;
        let base_contents = std::fs::read_to_string(base)?;
        let base_libretto: BaseLibretto = serde_json::from_str(&base_contents)?;
        validate_timing_overlay(&overlay, &base_libretto)?;
        tracing::info!("Timing overlay is valid");
    } else {
        // Try as base libretto first, then as timing overlay
        if let Ok(libretto) = serde_json::from_str::<BaseLibretto>(&contents) {
            validate_base_libretto(&libretto)?;
            tracing::info!("Base libretto is valid");
        } else if let Ok(overlay) = serde_json::from_str::<TimingOverlay>(&contents) {
            validate_timing_overlay_standalone(&overlay)?;
            tracing::info!("Timing overlay is valid (standalone, no base libretto cross-check)");
        } else {
            anyhow::bail!("File does not parse as a base libretto or timing overlay");
        }
    }

    Ok(())
}

/// Validate a base libretto for internal consistency.
pub fn validate_base_libretto(libretto: &BaseLibretto) -> Result<Vec<ValidationError>> {
    let mut errors = Vec::new();

    // Required fields
    if libretto.opera.title.is_empty() {
        errors.push(ValidationError::MissingField("opera.title".into()));
    }
    if libretto.opera.composer.is_empty() {
        errors.push(ValidationError::MissingField("opera.composer".into()));
    }
    if libretto.opera.language.is_empty() {
        errors.push(ValidationError::MissingField("opera.language".into()));
    }

    // Unique segment IDs
    let mut seen_ids = HashSet::new();
    for number in &libretto.numbers {
        if number.id.is_empty() {
            errors.push(ValidationError::MissingField(format!(
                "number.id (label: {})",
                number.label
            )));
        }
        for segment in &number.segments {
            if !seen_ids.insert(&segment.id) {
                errors.push(ValidationError::DuplicateSegmentId(segment.id.clone()));
            }
        }
    }

    if !errors.is_empty() {
        for e in &errors {
            tracing::warn!("{e}");
        }
    }

    Ok(errors)
}

/// Validate a timing overlay against a base libretto.
pub fn validate_timing_overlay(
    overlay: &TimingOverlay,
    base: &BaseLibretto,
) -> Result<Vec<ValidationError>> {
    let mut errors = validate_timing_overlay_standalone(overlay)?;

    // Check that all referenced segment IDs exist in the base libretto
    let base_seg_ids: HashSet<&str> = base.segment_ids().into_iter().collect();
    for track in &overlay.track_timings {
        for st in &track.segment_times {
            if !base_seg_ids.contains(st.segment_id.as_str()) {
                errors.push(ValidationError::UnknownSegmentId(st.segment_id.clone()));
            }
        }
    }

    // Number coverage analysis
    let base_number_ids: HashSet<&str> = base.numbers.iter().map(|n| n.id.as_str()).collect();
    let covered: HashSet<&str> = overlay.covered_number_ids().into_iter().collect();
    let omitted: HashSet<&str> = overlay.omitted_number_ids().into_iter().collect();

    // Check for omitted numbers that don't exist in the base
    for id in &omitted {
        if !base_number_ids.contains(id) {
            errors.push(ValidationError::UnknownOmittedNumber(id.to_string()));
        }
    }

    // Check for numbers that are both covered and omitted
    for id in covered.intersection(&omitted) {
        errors.push(ValidationError::ConflictingCoverage(id.to_string()));
    }

    // Check for unaccounted numbers (neither covered nor omitted)
    let accounted: HashSet<&str> = covered.union(&omitted).copied().collect();
    let mut unaccounted: Vec<&str> = base_number_ids.difference(&accounted).copied().collect();
    unaccounted.sort();
    for id in &unaccounted {
        errors.push(ValidationError::UnaccountedNumber(id.to_string()));
    }

    // Log coverage summary
    let coverage = CoverageReport {
        total: base_number_ids.len(),
        covered: covered.len(),
        omitted: omitted.len(),
        unaccounted: unaccounted.len(),
    };
    tracing::info!(
        total = coverage.total,
        covered = coverage.covered,
        omitted = coverage.omitted,
        unaccounted = coverage.unaccounted,
        "Number coverage"
    );

    if !errors.is_empty() {
        for e in &errors {
            tracing::warn!("{e}");
        }
    }

    Ok(errors)
}

/// Summary of how well a timing overlay covers the base libretto.
#[derive(Debug, Clone)]
pub struct CoverageReport {
    pub total: usize,
    pub covered: usize,
    pub omitted: usize,
    pub unaccounted: usize,
}

/// Validate a timing overlay for internal consistency (without a base libretto).
pub fn validate_timing_overlay_standalone(
    overlay: &TimingOverlay,
) -> Result<Vec<ValidationError>> {
    let mut errors = Vec::new();

    for track in &overlay.track_timings {
        // Check segment times are ordered
        let mut prev_start = -1.0_f64;
        for st in &track.segment_times {
            if st.start < 0.0 {
                errors.push(ValidationError::NegativeTime(st.start));
            }
            if st.start < prev_start {
                errors.push(ValidationError::SegmentsUnordered(
                    track.track_title.clone(),
                ));
            }
            prev_start = st.start;
        }
    }

    Ok(errors)
}

#[cfg(test)]
mod tests {
    use super::*;
    use libretto_model::*;

    fn sample_libretto() -> BaseLibretto {
        let mut libretto = BaseLibretto::new(OperaMetadata {
            title: "Test Opera".to_string(),
            composer: "Test Composer".to_string(),
            librettist: None,
            language: "it".to_string(),
            translation_language: None,
            year: None,
        });
        libretto.numbers.push(MusicalNumber {
            id: "no-1".to_string(),
            label: "No. 1".to_string(),
            number_type: NumberType::Aria,
            act: "1".to_string(),
            scene: None,
            segments: vec![
                Segment {
                    id: "no-1-001".to_string(),
                    segment_type: SegmentType::Sung,
                    character: Some("TEST".to_string()),
                    text: Some("Test text".to_string()),
                    translation: None,
                    direction: None,
                },
                Segment {
                    id: "no-1-002".to_string(),
                    segment_type: SegmentType::Sung,
                    character: Some("TEST".to_string()),
                    text: Some("More text".to_string()),
                    translation: None,
                    direction: None,
                },
            ],
        });
        libretto
    }

    #[test]
    fn test_valid_libretto() {
        let libretto = sample_libretto();
        let errors = validate_base_libretto(&libretto).unwrap();
        assert!(errors.is_empty());
    }

    #[test]
    fn test_duplicate_segment_id() {
        let mut libretto = sample_libretto();
        libretto.numbers[0].segments[1].id = "no-1-001".to_string(); // duplicate
        let errors = validate_base_libretto(&libretto).unwrap();
        assert!(errors.iter().any(|e| matches!(e, ValidationError::DuplicateSegmentId(_))));
    }

    #[test]
    fn test_missing_title() {
        let mut libretto = sample_libretto();
        libretto.opera.title = String::new();
        let errors = validate_base_libretto(&libretto).unwrap();
        assert!(errors.iter().any(|e| matches!(e, ValidationError::MissingField(_))));
    }

    #[test]
    fn test_overlay_unknown_segment() {
        let libretto = sample_libretto();
        let overlay = TimingOverlay {
            version: "1.0".to_string(),
            base_libretto: "test".to_string(),
            recording: RecordingMetadata {
                conductor: None,
                orchestra: None,
                year: None,
                label: None,
                album_title: None,
            },
            contributors: vec![],
            omitted_numbers: vec![],
            track_timings: vec![TrackTiming {
                track_title: "Track 1".to_string(),
                disc_number: None,
                track_number: None,
                duration_seconds: None,
                number_ids: vec!["no-1".to_string()],
                start_segment_id: None,
                segment_times: vec![
                    SegmentTime { segment_id: "no-1-001".to_string(), start: 0.0 },
                    SegmentTime { segment_id: "no-1-999".to_string(), start: 5.0 }, // unknown
                ],
            }],
        };
        let errors = validate_timing_overlay(&overlay, &libretto).unwrap();
        assert!(errors.iter().any(|e| matches!(e, ValidationError::UnknownSegmentId(_))));
    }

    #[test]
    fn test_overlay_unordered_segments() {
        let overlay = TimingOverlay {
            version: "1.0".to_string(),
            base_libretto: "test".to_string(),
            recording: RecordingMetadata {
                conductor: None,
                orchestra: None,
                year: None,
                label: None,
                album_title: None,
            },
            contributors: vec![],
            omitted_numbers: vec![],
            track_timings: vec![TrackTiming {
                track_title: "Track 1".to_string(),
                disc_number: None,
                track_number: None,
                duration_seconds: None,
                number_ids: vec![],
                start_segment_id: None,
                segment_times: vec![
                    SegmentTime { segment_id: "a".to_string(), start: 10.0 },
                    SegmentTime { segment_id: "b".to_string(), start: 5.0 }, // out of order
                ],
            }],
        };
        let errors = validate_timing_overlay_standalone(&overlay).unwrap();
        assert!(errors.iter().any(|e| matches!(e, ValidationError::SegmentsUnordered(_))));
    }

    #[test]
    fn test_unaccounted_number() {
        // Base has "no-1" but overlay doesn't cover or omit it
        let libretto = sample_libretto();
        let overlay = TimingOverlay {
            version: "1.0".to_string(),
            base_libretto: "test".to_string(),
            recording: RecordingMetadata {
                conductor: None, orchestra: None, year: None, label: None, album_title: None,
            },
            contributors: vec![],
            omitted_numbers: vec![],
            track_timings: vec![], // no tracks at all
        };
        let errors = validate_timing_overlay(&overlay, &libretto).unwrap();
        assert!(errors.iter().any(|e| matches!(e, ValidationError::UnaccountedNumber(_))));
    }

    #[test]
    fn test_omitted_number_valid() {
        // Base has "no-1", overlay declares it omitted — should be clean
        let libretto = sample_libretto();
        let overlay = TimingOverlay {
            version: "1.0".to_string(),
            base_libretto: "test".to_string(),
            recording: RecordingMetadata {
                conductor: None, orchestra: None, year: None, label: None, album_title: None,
            },
            contributors: vec![],
            omitted_numbers: vec![OmittedNumber {
                number_id: "no-1".to_string(),
                reason: Some("Traditional cut".to_string()),
            }],
            track_timings: vec![],
        };
        let errors = validate_timing_overlay(&overlay, &libretto).unwrap();
        assert!(errors.is_empty(), "Expected no errors, got: {:?}", errors);
    }

    #[test]
    fn test_conflicting_coverage() {
        // Number is both covered by a track AND declared omitted
        let libretto = sample_libretto();
        let overlay = TimingOverlay {
            version: "1.0".to_string(),
            base_libretto: "test".to_string(),
            recording: RecordingMetadata {
                conductor: None, orchestra: None, year: None, label: None, album_title: None,
            },
            contributors: vec![],
            omitted_numbers: vec![OmittedNumber {
                number_id: "no-1".to_string(),
                reason: None,
            }],
            track_timings: vec![TrackTiming {
                track_title: "Track 1".to_string(),
                disc_number: None,
                track_number: None,
                duration_seconds: None,
                number_ids: vec!["no-1".to_string()],
                start_segment_id: None,
                segment_times: vec![],
            }],
        };
        let errors = validate_timing_overlay(&overlay, &libretto).unwrap();
        assert!(errors.iter().any(|e| matches!(e, ValidationError::ConflictingCoverage(_))));
    }

    #[test]
    fn test_unknown_omitted_number() {
        // Overlay declares a number omitted that doesn't exist in the base
        let libretto = sample_libretto();
        let overlay = TimingOverlay {
            version: "1.0".to_string(),
            base_libretto: "test".to_string(),
            recording: RecordingMetadata {
                conductor: None, orchestra: None, year: None, label: None, album_title: None,
            },
            contributors: vec![],
            omitted_numbers: vec![OmittedNumber {
                number_id: "no-99-nonexistent".to_string(),
                reason: None,
            }],
            track_timings: vec![TrackTiming {
                track_title: "Track 1".to_string(),
                disc_number: None,
                track_number: None,
                duration_seconds: None,
                number_ids: vec!["no-1".to_string()],
                start_segment_id: None,
                segment_times: vec![],
            }],
        };
        let errors = validate_timing_overlay(&overlay, &libretto).unwrap();
        assert!(errors.iter().any(|e| matches!(e, ValidationError::UnknownOmittedNumber(_))));
    }
}
