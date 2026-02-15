// Resolve track-to-segment boundaries by matching quoted anchor text
// from track titles against segment text in the base libretto.
//
// Track titles in opera recordings typically contain quoted text snippets
// that correspond to the opening words of segments. For example:
//   "Recitativo \"Bravo, signor padrone\"; No. 3 Cavatina \"Se vuol ballare\""
// The first quoted string tells us which segment starts this track.
//
// This module extracts those anchors, matches them to segments, and
// populates `start_segment_id` on each TrackTiming.

use unicode_normalization::UnicodeNormalization;

use crate::base_libretto::BaseLibretto;
use crate::timing_overlay::TimingOverlay;

/// Result of anchor resolution.
#[derive(Debug)]
pub struct ResolveResult {
    /// The overlay with `start_segment_id` populated where matches were found.
    pub overlay: TimingOverlay,
    /// Per-track resolution details.
    pub resolutions: Vec<TrackResolution>,
    /// Warnings for unresolved or ambiguous anchors.
    pub warnings: Vec<String>,
}

/// Resolution details for a single track.
#[derive(Debug)]
pub struct TrackResolution {
    pub track_title: String,
    pub disc_number: Option<u32>,
    pub track_number: Option<u32>,
    /// The anchors extracted from the track title.
    pub anchors: Vec<String>,
    /// The first anchor's matched segment ID (becomes start_segment_id).
    pub resolved_segment_id: Option<String>,
    /// How the match was made.
    pub match_method: Option<MatchMethod>,
}

/// How an anchor was matched to a segment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchMethod {
    /// Exact prefix match on first line of segment text.
    PrefixMatch,
    /// Match found after accent/punctuation normalization.
    NormalizedMatch,
    /// Match found via substring search within segment text.
    SubstringMatch,
    /// Anchor was already set manually (preserved).
    Manual,
}

/// Extract quoted strings from a track title.
/// Handles both straight quotes and typographic quotes.
pub(crate) fn extract_anchors(title: &str) -> Vec<String> {
    let mut anchors = Vec::new();
    let mut chars = title.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '"' || c == '\u{201c}' {
            // Collect until closing quote
            let close = if c == '\u{201c}' { '\u{201d}' } else { '"' };
            let mut quoted = String::new();
            for ch in chars.by_ref() {
                if ch == close || ch == '"' {
                    break;
                }
                quoted.push(ch);
            }
            let trimmed = quoted.trim().to_string();
            if !trimmed.is_empty() {
                anchors.push(trimmed);
            }
        }
    }

    anchors
}

/// Normalize text for fuzzy matching: lowercase, strip accents, normalize punctuation.
fn normalize_for_match(text: &str) -> String {
    text.nfd()
        .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
        .collect::<String>()
        .to_lowercase()
        .replace('\u{2018}', "'")
        .replace('\u{2019}', "'")
        .replace('\u{201c}', "\"")
        .replace('\u{201d}', "\"")
        .replace("...", "…")
        .replace([',', ';', ':', '!', '?'], "")
        .replace("  ", " ")
        .trim()
        .to_string()
}

/// A classified anchor from a track title, tagged as recitative or not.
#[derive(Debug, Clone)]
pub struct TitleAnchor {
    /// Whether this anchor is in a recitative section of the title.
    pub is_recitative: bool,
    /// The quoted anchor text.
    pub anchor: String,
}

/// Parse a track title and classify each quoted anchor as recitative or not.
///
/// Examines the text preceding each quoted string to determine if it falls
/// under a "recitativo" label. Keywords like "aria", "duetto", "cavatina"
/// indicate non-recitative (sung) sections.
pub fn classify_title_anchors(title: &str) -> Vec<TitleAnchor> {
    let anchors = extract_anchors(title);
    let mut result = Vec::new();
    let mut search_from = 0;

    for anchor in &anchors {
        if let Some(pos) = title[search_from..].find(anchor.as_str()) {
            let abs_pos = search_from + pos;
            let context = title[search_from..abs_pos].to_lowercase();
            result.push(TitleAnchor {
                is_recitative: is_recitative_context(&context),
                anchor: anchor.clone(),
            });
            search_from = abs_pos + anchor.len();
        }
    }

    result
}

/// Check whether the context text preceding a quoted anchor indicates recitative.
///
/// Returns true if "recitativ" appears and is the last type-indicating keyword
/// (i.e., no aria/duet/etc. keyword appears after it).
fn is_recitative_context(context: &str) -> bool {
    let recit_pos = context.rfind("recitativ");
    let sung_keywords = [
        "aria", "duett", "cavatina", "canzon", "terzett",
        "quartett", "quintett", "sestett", "finale", "coro",
        "sinfonia", "marcia",
    ];
    let last_sung_pos = sung_keywords.iter()
        .filter_map(|kw| context.rfind(kw))
        .max();

    match (recit_pos, last_sung_pos) {
        (Some(rp), Some(sp)) => rp > sp,
        (Some(_), None) => true,
        _ => false,
    }
}

/// A candidate segment for matching.
pub(crate) struct SegCandidate<'a> {
    segment_id: &'a str,
    number_id: &'a str,
    first_line: String,
    full_text: String,
    first_line_norm: String,
    full_text_norm: String,
}

/// Build a searchable index of all segments with text.
pub(crate) fn build_segment_index(base: &BaseLibretto) -> Vec<SegCandidate<'_>> {
    let mut candidates = Vec::new();
    for number in &base.numbers {
        for seg in &number.segments {
            if let Some(text) = &seg.text {
                let first_line = text.split('\n').next().unwrap_or("").to_string();
                let full_text = text.clone();
                let first_line_norm = normalize_for_match(&first_line);
                let full_text_norm = normalize_for_match(&full_text);
                candidates.push(SegCandidate {
                    segment_id: &seg.id,
                    number_id: &number.id,
                    first_line,
                    full_text,
                    first_line_norm,
                    full_text_norm,
                });
            }
        }
    }
    candidates
}

/// Take the first N chars of a string (char-safe, no byte-boundary panics).
fn char_prefix(s: &str, n: usize) -> &str {
    match s.char_indices().nth(n) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}

/// Try to match an anchor to a segment, preferring matches within the given number_ids.
pub(crate) fn match_anchor(
    anchor: &str,
    number_ids: &[String],
    candidates: &[SegCandidate<'_>],
) -> Option<(String, MatchMethod)> {
    let anchor_norm = normalize_for_match(anchor);
    let anchor_prefix = char_prefix(&anchor_norm, 15);

    // Strategy 1: Prefix match on first line (exact, within number_ids first)
    for pass in &[true, false] {
        let filter_nids = *pass;
        for cand in candidates {
            if filter_nids && !number_ids.contains(&cand.number_id.to_string()) {
                continue;
            }
            let cand_prefix = char_prefix(&cand.first_line_norm, 15);
            if cand.first_line_norm.starts_with(anchor_prefix)
                || anchor_norm.starts_with(cand_prefix)
            {
                return Some((cand.segment_id.to_string(), MatchMethod::PrefixMatch));
            }
        }
    }

    // Strategy 2: Normalized match on first line (after accent stripping)
    for pass in &[true, false] {
        let filter_nids = *pass;
        for cand in candidates {
            if filter_nids && !number_ids.contains(&cand.number_id.to_string()) {
                continue;
            }
            if cand.first_line_norm.contains(&anchor_norm) {
                return Some((cand.segment_id.to_string(), MatchMethod::NormalizedMatch));
            }
        }
    }

    // Strategy 3: Substring match anywhere in full text
    for pass in &[true, false] {
        let filter_nids = *pass;
        for cand in candidates {
            if filter_nids && !number_ids.contains(&cand.number_id.to_string()) {
                continue;
            }
            if cand.full_text_norm.contains(&anchor_norm) {
                return Some((cand.segment_id.to_string(), MatchMethod::SubstringMatch));
            }
        }
    }

    None
}

/// Resolve track title anchors to segment IDs.
///
/// For each track in the overlay:
/// 1. If `start_segment_id` is already set, preserve it (manual override).
/// 2. Extract quoted text from the track title.
/// 3. Match the first anchor to a segment in the base libretto.
/// 4. Set `start_segment_id` to the matched segment ID.
///
/// The first anchor in the track title is used as the start segment because
/// it typically corresponds to the opening text of that track.
pub fn resolve_anchors(base: &BaseLibretto, overlay: &TimingOverlay) -> ResolveResult {
    let mut result_overlay = overlay.clone();
    let mut resolutions = Vec::new();
    let mut warnings = Vec::new();
    let candidates = build_segment_index(base);

    for (i, track) in overlay.track_timings.iter().enumerate() {
        let anchors = extract_anchors(&track.track_title);

        // Preserve manual overrides
        if track.start_segment_id.is_some() {
            resolutions.push(TrackResolution {
                track_title: track.track_title.clone(),
                disc_number: track.disc_number,
                track_number: track.track_number,
                anchors,
                resolved_segment_id: track.start_segment_id.clone(),
                match_method: Some(MatchMethod::Manual),
            });
            continue;
        }

        if anchors.is_empty() {
            // No quoted text — use first segment of the first referenced number
            let fallback = track.number_ids.first()
                .and_then(|nid| base.find_number(nid))
                .and_then(|n| n.segments.first())
                .map(|s| s.id.clone());

            if let Some(seg_id) = &fallback {
                result_overlay.track_timings[i].start_segment_id = Some(seg_id.clone());
            }

            resolutions.push(TrackResolution {
                track_title: track.track_title.clone(),
                disc_number: track.disc_number,
                track_number: track.track_number,
                anchors: vec![],
                resolved_segment_id: fallback,
                match_method: None,
            });
            continue;
        }

        // Try to match the first anchor — it determines the track's start segment
        // Also collect number_ids from this track AND adjacent tracks for broader search
        let mut search_nids = track.number_ids.clone();
        // Include number_ids from the previous track (anchor might be tail of prev number)
        if i > 0 {
            for nid in &overlay.track_timings[i - 1].number_ids {
                if !search_nids.contains(nid) {
                    search_nids.push(nid.clone());
                }
            }
        }

        let first_anchor = &anchors[0];
        let matched = match_anchor(first_anchor, &search_nids, &candidates);

        match &matched {
            Some((seg_id, method)) => {
                result_overlay.track_timings[i].start_segment_id = Some(seg_id.clone());
                resolutions.push(TrackResolution {
                    track_title: track.track_title.clone(),
                    disc_number: track.disc_number,
                    track_number: track.track_number,
                    anchors,
                    resolved_segment_id: Some(seg_id.clone()),
                    match_method: Some(method.clone()),
                });
            }
            None => {
                warnings.push(format!(
                    "D{}T{}: anchor \"{}\" — no match found in base libretto",
                    track.disc_number.unwrap_or(0),
                    track.track_number.unwrap_or(0),
                    first_anchor,
                ));
                resolutions.push(TrackResolution {
                    track_title: track.track_title.clone(),
                    disc_number: track.disc_number,
                    track_number: track.track_number,
                    anchors,
                    resolved_segment_id: None,
                    match_method: None,
                });
            }
        }
    }

    ResolveResult {
        overlay: result_overlay,
        resolutions,
        warnings,
    }
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
            number_type: NumberType::Duettino,
            act: "1".to_string(),
            scene: None,
            segments: vec![
                Segment {
                    id: "no-1-001".to_string(),
                    segment_type: SegmentType::Sung,
                    character: Some("A".to_string()),
                    text: Some("Se a caso madama la notte ti chiama".to_string()),
                    translation: None,
                    direction: None,
                    group: None,
                },
                Segment {
                    id: "no-1-002".to_string(),
                    segment_type: SegmentType::Sung,
                    character: Some("B".to_string()),
                    text: Some("Or bene, ascolta, e taci".to_string()),
                    translation: None,
                    direction: None,
                    group: None,
                },
                Segment {
                    id: "no-1-003".to_string(),
                    segment_type: SegmentType::Sung,
                    character: Some("A".to_string()),
                    text: Some("Bravo, signor padrone! Ora incomincio".to_string()),
                    translation: None,
                    direction: None,
                    group: None,
                },
            ],
        });
        lib.numbers.push(MusicalNumber {
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
                    text: Some("Se vuol ballare, signor contino".to_string()),
                    translation: None,
                    direction: None,
                    group: None,
                },
            ],
        });
        lib
    }

    #[test]
    fn test_extract_anchors() {
        let title = r#"No. 2 Duetto "Se a caso madama"; recitativo "Or bene, ascolta""#;
        let anchors = extract_anchors(title);
        assert_eq!(anchors, vec!["Se a caso madama", "Or bene, ascolta"]);
    }

    #[test]
    fn test_extract_anchors_no_quotes() {
        let anchors = extract_anchors("Sinfonia");
        assert!(anchors.is_empty());
    }

    #[test]
    fn test_resolve_basic() {
        let base = test_base();
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
                    track_title: r#"No. 1 Duetto "Se a caso madama"; recitativo "Or bene, ascolta""#.to_string(),
                    disc_number: Some(1),
                    track_number: Some(1),
                    duration_seconds: Some(200.0),
                    number_ids: vec!["no-1".to_string()],
                    start_segment_id: None,
                    segment_times: vec![],
                },
                TrackTiming {
                    track_title: r#"Recitativo "Bravo, signor padrone"; No. 2 Cavatina "Se vuol ballare""#.to_string(),
                    disc_number: Some(1),
                    track_number: Some(2),
                    duration_seconds: Some(250.0),
                    number_ids: vec!["no-2".to_string()],
                    start_segment_id: None,
                    segment_times: vec![],
                },
            ],
        };

        let result = resolve_anchors(&base, &overlay);
        assert!(result.warnings.is_empty(), "warnings: {:?}", result.warnings);

        // Track 1 starts with "Se a caso madama" -> no-1-001
        assert_eq!(
            result.overlay.track_timings[0].start_segment_id.as_deref(),
            Some("no-1-001")
        );

        // Track 2 starts with "Bravo, signor padrone" -> no-1-003
        // (crossover: this segment is in no-1, but it's the start of track 2)
        assert_eq!(
            result.overlay.track_timings[1].start_segment_id.as_deref(),
            Some("no-1-003")
        );
    }

    #[test]
    fn test_resolve_preserves_manual() {
        let base = test_base();
        let overlay = TimingOverlay {
            version: "1.0".to_string(),
            base_libretto: "test".to_string(),
            recording: RecordingMetadata {
                conductor: None, orchestra: None, year: None, label: None, album_title: None,
            },
            contributors: vec![],
            omitted_numbers: vec![],
            track_timings: vec![TrackTiming {
                track_title: r#"No. 1 Duetto "Se a caso madama""#.to_string(),
                disc_number: Some(1),
                track_number: Some(1),
                duration_seconds: Some(200.0),
                number_ids: vec!["no-1".to_string()],
                start_segment_id: Some("no-1-002".to_string()), // manual override
                segment_times: vec![],
            }],
        };

        let result = resolve_anchors(&base, &overlay);
        // Should preserve the manual value
        assert_eq!(
            result.overlay.track_timings[0].start_segment_id.as_deref(),
            Some("no-1-002")
        );
        assert_eq!(result.resolutions[0].match_method, Some(MatchMethod::Manual));
    }

    #[test]
    fn test_resolve_no_quotes_fallback() {
        let base = test_base();
        let overlay = TimingOverlay {
            version: "1.0".to_string(),
            base_libretto: "test".to_string(),
            recording: RecordingMetadata {
                conductor: None, orchestra: None, year: None, label: None, album_title: None,
            },
            contributors: vec![],
            omitted_numbers: vec![],
            track_timings: vec![TrackTiming {
                track_title: "Sinfonia".to_string(),
                disc_number: Some(1),
                track_number: Some(1),
                duration_seconds: Some(200.0),
                number_ids: vec!["no-1".to_string()],
                start_segment_id: None,
                segment_times: vec![],
            }],
        };

        let result = resolve_anchors(&base, &overlay);
        // Should fall back to first segment of no-1
        assert_eq!(
            result.overlay.track_timings[0].start_segment_id.as_deref(),
            Some("no-1-001")
        );
    }

    #[test]
    fn test_normalize_for_match() {
        // Accented vs unaccented
        assert_eq!(
            normalize_for_match("perchè"),
            normalize_for_match("perche")
        );
        // Smart quotes
        assert_eq!(
            normalize_for_match("Crudel\u{2019}s"),
            normalize_for_match("Crudel's")
        );
    }

    #[test]
    fn test_classify_title_anchors_mixed() {
        let title = r#"Recitativo "Bravo, signor padrone"; No. 3 Cavatina "Se vuol ballare"; recitativo "Ed aspettaste il giorno""#;
        let anchors = classify_title_anchors(title);
        assert_eq!(anchors.len(), 3);
        assert!(anchors[0].is_recitative);
        assert_eq!(anchors[0].anchor, "Bravo, signor padrone");
        assert!(!anchors[1].is_recitative);
        assert_eq!(anchors[1].anchor, "Se vuol ballare");
        assert!(anchors[2].is_recitative);
        assert_eq!(anchors[2].anchor, "Ed aspettaste il giorno");
    }

    #[test]
    fn test_classify_title_anchors_recit_then_aria() {
        // "No. 17 Recitativo ... ed Aria ..." has two anchors in one section
        let title = r#"No. 17 Recitativo "Hai già vinta la causa?" ed Aria "Vedrò, mentr'io sospiro""#;
        let anchors = classify_title_anchors(title);
        assert_eq!(anchors.len(), 2);
        assert!(anchors[0].is_recitative);
        assert!(!anchors[1].is_recitative);
    }

    #[test]
    fn test_classify_title_anchors_no_quotes() {
        let anchors = classify_title_anchors("Sinfonia");
        assert!(anchors.is_empty());
    }

    #[test]
    fn test_classify_title_anchors_aria_only() {
        let title = r#"No. 9 Aria "Non più andrai""#;
        let anchors = classify_title_anchors(title);
        assert_eq!(anchors.len(), 1);
        assert!(!anchors[0].is_recitative);
    }
}
