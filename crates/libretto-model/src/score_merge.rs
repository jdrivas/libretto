// Merge a grouped score libretto with recording timing data (Approach A).
//
// Produces a track-centric output: each track from the recording becomes
// the primary record, with matched score numbers nested inside it.
//
// The algorithm:
// 1. Extract the first quoted anchor from each track title.
// 2. Search score numbers for a first_words, title-quote, or segment-text match.
// 3. This defines "boundaries" — which score number starts each track.
// 4. Assign all score numbers between consecutive boundaries to each track.

use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;

use crate::resolve::extract_anchors;
use crate::timing_overlay::{Contributor, RecordingMetadata, TimingOverlay};

// ============================================================
// Input types: Grouped Score Libretto
// ============================================================

/// A grouped score libretto: score-level structure with libretto text merged in.
/// Produced by the generate pipeline from score page descriptions + bilingual libretto.
#[derive(Debug, Clone, Deserialize)]
pub struct GroupedScoreLibretto {
    pub front_matter: ScoreFrontMatter,
    pub numbers: Vec<ScoreNumber>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreFrontMatter {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_pages: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub languages: Option<ScoreLanguages>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pages: Vec<ScoreFrontMatterPage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreLanguages {
    pub text: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub translation: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreFrontMatterPage {
    pub page: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
}

/// A single score number: may be an overture, aria, recitative, finale scene, etc.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreNumber {
    pub id: u32,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub act: Option<String>,
    #[serde(default)]
    pub content_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_words: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub characters: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pages: Vec<ScorePage>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub segments: Vec<ScoreSegment>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub libretto_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub libretto_act: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub libretto_characters: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub libretto_direction: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub libretto_form: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub libretto_number_ids: Vec<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScorePage {
    pub page: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub characters: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_words: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_words: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreSegment {
    pub id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub character: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub translation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
    /// Estimated start time in seconds within the track (filled by estimate step).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start: Option<f64>,
}

// ============================================================
// Output types: Merged track-centric structure
// ============================================================

/// Result of the score merge operation.
#[derive(Debug)]
pub struct ScoreMergeResult {
    pub output: ScoreMergedOutput,
    pub boundaries: Vec<BoundaryInfo>,
    pub warnings: Vec<String>,
    pub stats: ScoreMergeStats,
}

/// The merged output document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreMergedOutput {
    pub version: String,
    pub front_matter: ScoreFrontMatter,
    pub recording: RecordingMetadata,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discogs: Option<serde_json::Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub contributors: Vec<Contributor>,
    pub tracks: Vec<MergedTrack>,
}

/// A single track in the merged output, with score numbers nested inside.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergedTrack {
    pub track_title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disc_number: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track_number: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<f64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub characters: Vec<String>,
    pub score_numbers: Vec<ScoreNumber>,
}

/// Information about how a track boundary was matched.
#[derive(Debug, Clone)]
pub struct BoundaryInfo {
    pub track_idx: usize,
    pub disc_number: Option<u32>,
    pub track_number: Option<u32>,
    pub score_number_id: u32,
    pub method: String,
}

#[derive(Debug, Clone)]
pub struct ScoreMergeStats {
    pub total_tracks: usize,
    pub matched_tracks: usize,
    pub total_score_numbers: usize,
    pub assigned_score_numbers: usize,
    pub total_segments: usize,
}

// ============================================================
// Text matching utilities
// ============================================================

/// Normalize text for score merge matching.
/// Strips hyphens (OCR syllable breaks), collapses whitespace,
/// removes accents, lowercases, and strips punctuation.
fn normalize_for_score_match(text: &str) -> String {
    let dehyphenated = text.replace('-', "");
    // Collapse all whitespace (including newlines) to single space
    let collapsed: String = dehyphenated
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    // Strip accents via NFD decomposition
    let stripped: String = collapsed
        .nfd()
        .filter(|c| !unicode_normalization::char::is_combining_mark(*c))
        .collect::<String>()
        .to_lowercase();
    // Remove punctuation
    stripped
        .replace([',', ';', ':', '!', '?', '.', '\'', '"', '\u{2018}', '\u{2019}',
                  '\u{201c}', '\u{201d}', '\u{2026}'], "")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Compact text: normalize then strip ALL whitespace.
/// Handles OCR syllable splits like "Cru del" → "crudel".
fn compact_for_score_match(text: &str) -> String {
    normalize_for_score_match(text)
        .chars()
        .filter(|c| !c.is_whitespace())
        .collect()
}

/// Take the first N chars of a string (char-safe).
fn char_prefix(s: &str, n: usize) -> &str {
    match s.char_indices().nth(n) {
        Some((idx, _)) => &s[..idx],
        None => s,
    }
}

/// Check if two texts match by normalized prefix.
/// Tries space-preserving match first, then compact (spaceless) match.
fn text_prefix_match(text1: &str, text2: &str) -> bool {
    let min_chars = 4;

    // Normal prefix comparison
    let n1 = normalize_for_score_match(text1);
    let n2 = normalize_for_score_match(text2);
    let compare_len = n1.len().min(n2.len()).min(20);
    if compare_len >= min_chars {
        let p1 = char_prefix(&n1, compare_len);
        let p2 = char_prefix(&n2, compare_len);
        if p1 == p2 {
            return true;
        }
    }

    // Compact fallback (handles OCR syllable splits)
    let c1 = compact_for_score_match(text1);
    let c2 = compact_for_score_match(text2);
    let compare_len = c1.len().min(c2.len()).min(18);
    if compare_len >= min_chars {
        let p1 = char_prefix(&c1, compare_len);
        let p2 = char_prefix(&c2, compare_len);
        if p1 == p2 {
            return true;
        }
    }

    false
}

// ============================================================
// Boundary detection
// ============================================================

/// Internal boundary record during detection.
#[derive(Debug, Clone)]
struct Boundary {
    track_idx: usize,
    score_idx: usize,
    method: String,
}

/// Check if a track anchor matches a score number's first_words, title quotes,
/// or (optionally) segment text. Returns a method description on match.
fn find_anchor_in_score_number(
    sn: &ScoreNumber,
    anchor: &str,
    search_segments: bool,
) -> Option<String> {
    // Check first_words
    if let Some(fw) = &sn.first_words {
        if text_prefix_match(fw, anchor) {
            return Some("first_words".to_string());
        }
    }

    // Check quoted text in the score title (clean text, no OCR artifacts)
    for quoted in extract_anchors(&sn.title) {
        if text_prefix_match(&quoted, anchor) {
            return Some("title_quote".to_string());
        }
    }

    // Check segment text (for finales where matching text is mid-number)
    if search_segments {
        for seg in &sn.segments {
            if let Some(text) = &seg.text {
                if text_prefix_match(text, anchor) {
                    return Some(format!("segment:{}", seg.id));
                }
            }
        }
    }

    None
}

/// Detect boundary positions: which score number starts each track.
///
/// Two passes:
///   Pass 1: first_words + title quotes only (reliable, no crossover risk)
///   Pass 2: also search segment text (needed for finales/multi-part numbers)
///
/// Special case: overture detection for the first track.
fn detect_boundaries(
    tracks: &[crate::timing_overlay::TrackTiming],
    score_numbers: &[ScoreNumber],
) -> Vec<Boundary> {
    let mut boundaries: Vec<Boundary> = Vec::new();

    for search_segments in [false, true] {
        for (track_idx, track) in tracks.iter().enumerate() {
            // Skip if already matched
            if boundaries.iter().any(|b| b.track_idx == track_idx) {
                continue;
            }

            // Special case: overture (first track, no quoted anchors)
            if track_idx == 0 && !score_numbers.is_empty() {
                let first_sn = &score_numbers[0];
                let anchors = extract_anchors(&track.track_title);
                if anchors.is_empty() && first_sn.content_type == "overture" {
                    let title_lower = track.track_title.to_lowercase();
                    if title_lower.contains("sinfonia")
                        || title_lower.contains("overture")
                        || title_lower.contains("ouverture")
                        || title_lower.contains("ouvertüre")
                        || title_lower.contains("preludio")
                    {
                        boundaries.push(Boundary {
                            track_idx: 0,
                            score_idx: 0,
                            method: "overture".to_string(),
                        });
                        continue;
                    }
                }
            }

            let anchors = extract_anchors(&track.track_title);
            if anchors.is_empty() {
                continue;
            }
            let first_anchor = &anchors[0];

            // Find the previous boundary's score index (for tracks before this one)
            let prev_sn_idx = boundaries
                .iter()
                .filter(|b| b.track_idx < track_idx)
                .map(|b| b.score_idx)
                .max()
                .map(|idx| idx + 1)
                .unwrap_or(0);

            // Search score numbers from prev_boundary onward
            for sn_idx in prev_sn_idx..score_numbers.len() {
                if let Some(method) = find_anchor_in_score_number(
                    &score_numbers[sn_idx],
                    first_anchor,
                    search_segments,
                ) {
                    boundaries.push(Boundary {
                        track_idx,
                        score_idx: sn_idx,
                        method: format!("{} -> '{}'", method, truncate(first_anchor, 40)),
                    });
                    break;
                }
            }
        }
    }

    // Sort by track_idx for stable processing
    boundaries.sort_by_key(|b| b.track_idx);
    boundaries
}

/// Truncate a string to at most `n` chars.
fn truncate(s: &str, n: usize) -> &str {
    char_prefix(s, n)
}

// ============================================================
// Main merge function
// ============================================================

/// Merge a grouped score libretto with recording timing data.
///
/// `discogs` is optional metadata from the timing file (passed as raw JSON
/// since its structure varies and it's not part of TimingOverlay).
pub fn score_merge(
    libretto: &GroupedScoreLibretto,
    timing: &TimingOverlay,
    discogs: Option<serde_json::Value>,
) -> ScoreMergeResult {
    let tracks = &timing.track_timings;
    let score_numbers = &libretto.numbers;
    let mut warnings = Vec::new();

    // --- Boundary detection ---
    let boundaries = detect_boundaries(tracks, score_numbers);

    // Validate monotonicity
    for pair in boundaries.windows(2) {
        if pair[1].score_idx <= pair[0].score_idx {
            warnings.push(format!(
                "boundary order conflict: track {} (score #{}) vs track {} (score #{})",
                pair[0].track_idx,
                score_numbers[pair[0].score_idx].id,
                pair[1].track_idx,
                score_numbers[pair[1].score_idx].id,
            ));
        }
    }

    // --- Assignment: score numbers between boundaries go to each track ---
    let mut assignments: Vec<Vec<ScoreNumber>> = vec![Vec::new(); tracks.len()];

    let sorted_track_idxs: Vec<usize> = boundaries.iter().map(|b| b.track_idx).collect();

    for (i, boundary) in boundaries.iter().enumerate() {
        let sn_start = boundary.score_idx;
        let sn_end = if i + 1 < boundaries.len() {
            boundaries[i + 1].score_idx
        } else {
            score_numbers.len()
        };

        for j in sn_start..sn_end {
            assignments[boundary.track_idx].push(score_numbers[j].clone());
        }
    }

    // Warn about unmatched tracks (no boundary and no assignments)
    for (track_idx, track) in tracks.iter().enumerate() {
        if !sorted_track_idxs.contains(&track_idx) && assignments[track_idx].is_empty() {
            warnings.push(format!(
                "D{}T{}: no boundary match for '{}'",
                track.disc_number.unwrap_or(0),
                track.track_number.unwrap_or(0),
                truncate(&track.track_title, 60),
            ));
        }
    }

    // --- Build output ---
    let mut total_segments = 0usize;
    let mut assigned_count = 0usize;

    let merged_tracks: Vec<MergedTrack> = tracks
        .iter()
        .enumerate()
        .map(|(idx, track)| {
            let score_nums = &assignments[idx];
            let segs: usize = score_nums.iter().map(|sn| sn.segments.len()).sum();
            total_segments += segs;
            assigned_count += score_nums.len();

            MergedTrack {
                track_title: track.track_title.clone(),
                disc_number: track.disc_number,
                track_number: track.track_number,
                duration_seconds: track.duration_seconds,
                characters: track.characters.clone(),
                score_numbers: score_nums.clone(),
            }
        })
        .collect();

    let boundary_infos: Vec<BoundaryInfo> = boundaries
        .iter()
        .map(|b| BoundaryInfo {
            track_idx: b.track_idx,
            disc_number: tracks[b.track_idx].disc_number,
            track_number: tracks[b.track_idx].track_number,
            score_number_id: score_numbers[b.score_idx].id,
            method: b.method.clone(),
        })
        .collect();

    ScoreMergeResult {
        output: ScoreMergedOutput {
            version: "1.0".to_string(),
            front_matter: libretto.front_matter.clone(),
            recording: timing.recording.clone(),
            discogs,
            contributors: timing.contributors.clone(),
            tracks: merged_tracks,
        },
        boundaries: boundary_infos,
        warnings,
        stats: ScoreMergeStats {
            total_tracks: tracks.len(),
            matched_tracks: sorted_track_idxs.len(),
            total_score_numbers: score_numbers.len(),
            assigned_score_numbers: assigned_count,
            total_segments,
        },
    }
}

// ============================================================
// Timing estimation for ScoreMergedOutput
// ============================================================

/// Recitative segments are spoken-sung at roughly 2× the pace of sung text.
const RECITATIVE_DISCOUNT: f64 = 0.5;

/// Word weight for a score segment's text.
/// Segments with no text (directions, interludes) get weight 0.
fn segment_word_weight(seg: &ScoreSegment) -> f64 {
    seg.text.as_deref()
        .map(|t| t.split_whitespace().count() as f64)
        .unwrap_or(0.0)
}

/// Whether a score number's content_type indicates recitative.
fn is_recitative(content_type: &str) -> bool {
    content_type == "recitative"
}

/// Round to millisecond precision.
fn round_to_ms(seconds: f64) -> f64 {
    (seconds * 1000.0).round() / 1000.0
}

/// Statistics from timing estimation.
#[derive(Debug)]
pub struct ScoreEstimateStats {
    pub tracks_estimated: usize,
    pub segments_estimated: usize,
}

/// A timing unit: either a single ungrouped segment or a collapsed group.
struct TimingUnit {
    /// (number_idx, segment_idx) for each segment in this unit.
    members: Vec<(usize, usize)>,
    /// Effective weight for duration allocation.
    weight: f64,
}

/// Estimate segment start times within each track of a ScoreMergedOutput.
///
/// Uses word-weight proportional distribution: each segment gets a fraction
/// of the track duration proportional to its word count, with recitative
/// segments discounted (faster pacing).
///
/// Group collapsing: consecutive segments sharing the same `group` tag are
/// treated as one timing unit weighted by max(word_weight) across members.
/// All members of a group receive the same start time (simultaneous singing).
///
/// Direction segments (no text) get weight 0 — they share the start time of
/// the next real segment.
///
/// Modifies the output in place, filling in `start` on each ScoreSegment.
pub fn estimate_score_merge_timings(output: &mut ScoreMergedOutput) -> (ScoreEstimateStats, Vec<String>) {
    let mut warnings = Vec::new();
    let mut tracks_estimated = 0usize;
    let mut segments_estimated = 0usize;

    for track in &mut output.tracks {
        let duration = match track.duration_seconds {
            Some(d) if d > 0.0 => d,
            _ => {
                warnings.push(format!(
                    "D{}T{} '{}': no duration, skipping timing estimation",
                    track.disc_number.unwrap_or(0),
                    track.track_number.unwrap_or(0),
                    truncate(&track.track_title, 40),
                ));
                continue;
            }
        };

        // Collect per-segment info: (number_idx, segment_idx, weight, group)
        struct SegInfo {
            ni: usize,
            si: usize,
            weight: f64,
            group: Option<String>,
        }

        let mut seg_infos: Vec<SegInfo> = Vec::new();

        for (ni, sn) in track.score_numbers.iter().enumerate() {
            let discount = if is_recitative(&sn.content_type) {
                RECITATIVE_DISCOUNT
            } else {
                1.0
            };

            for (si, seg) in sn.segments.iter().enumerate() {
                let w = segment_word_weight(seg) * discount;
                seg_infos.push(SegInfo {
                    ni, si, weight: w, group: seg.group.clone(),
                });
            }
        }

        if seg_infos.is_empty() {
            continue;
        }

        // Build timing units: collapse consecutive same-group segments
        let mut units: Vec<TimingUnit> = Vec::new();
        let mut i = 0;
        while i < seg_infos.len() {
            if let Some(ref g) = seg_infos[i].group {
                // Collect consecutive segments with the same group tag
                let start = i;
                let mut max_w = seg_infos[i].weight;
                i += 1;
                while i < seg_infos.len() && seg_infos[i].group.as_deref() == Some(g) {
                    max_w = max_w.max(seg_infos[i].weight);
                    i += 1;
                }
                let members: Vec<(usize, usize)> = seg_infos[start..i]
                    .iter()
                    .map(|s| (s.ni, s.si))
                    .collect();
                units.push(TimingUnit { members, weight: max_w });
            } else {
                // Ungrouped segment
                let s = &seg_infos[i];
                units.push(TimingUnit {
                    members: vec![(s.ni, s.si)],
                    weight: s.weight,
                });
                i += 1;
            }
        }

        let total_weight: f64 = units.iter().map(|u| u.weight).sum();
        if total_weight == 0.0 { continue; }

        let mut cumulative = 0.0;
        let mut seg_count = 0usize;
        for unit in &units {
            let start = round_to_ms((cumulative / total_weight) * duration);
            for &(ni, si) in &unit.members {
                track.score_numbers[ni].segments[si].start = Some(start);
                seg_count += 1;
            }
            cumulative += unit.weight;
        }

        tracks_estimated += 1;
        segments_estimated += seg_count;
    }

    let stats = ScoreEstimateStats { tracks_estimated, segments_estimated };
    (stats, warnings)
}

// ============================================================
// Flatten to InterchangeLibretto
// ============================================================

use crate::interchange::{InterchangeLibretto, InterchangeOpera, InterchangeTrack, InterchangeSegment};

/// Map a score number's content_type to an interchange segment type.
fn map_segment_type(content_type: &str) -> &'static str {
    match content_type {
        "recitative" => "recitative",
        "overture" => "interlude",
        _ => "sung",
    }
}

/// Convert a timed ScoreMergedOutput into an InterchangeLibretto.
///
/// Flattens `track.score_numbers[].segments[]` into a single segment list
/// per track, preserving timing. Injects act transitions and stage directions
/// as metadata on segments.
///
/// The ScoreMergedOutput must have timing already estimated (via
/// `estimate_score_merge_timings`) — segments without `start` are skipped.
pub fn score_merged_to_interchange(merged: &ScoreMergedOutput) -> InterchangeLibretto {
    // Extract opera metadata from front_matter
    let languages = merged.front_matter.languages.as_ref();
    let opera = InterchangeOpera {
        title: merged.front_matter.title.clone(),
        composer: String::new(), // Not in front_matter; consumer can fill in
        librettist: None,
        language: languages.map_or("it".to_string(), |l| l.text.clone()),
        translation_language: languages.and_then(|l| l.translation.clone()),
        year: merged.recording.year.and_then(|y| u16::try_from(y).ok()),
    };

    // Build interchange recording metadata
    let recording = crate::interchange::InterchangeRecording {
        album_title: merged.recording.album_title.clone(),
        album_title_aliases: merged.recording.album_title_aliases.clone(),
        artist: match (&merged.recording.conductor, &merged.recording.orchestra) {
            (Some(c), Some(o)) => Some(format!("{} / {}", c, o)),
            (Some(c), None) => Some(c.clone()),
            (None, Some(o)) => Some(o.clone()),
            (None, None) => None,
        },
        conductor: merged.recording.conductor.clone(),
        orchestra: merged.recording.orchestra.clone(),
        year: merged.recording.year,
        label: merged.recording.label.clone(),
    };

    let tracks: Vec<InterchangeTrack> = merged.tracks.iter().map(|track| {
        let track_id = format!(
            "d{}t{}",
            track.disc_number.unwrap_or(0),
            track.track_number.unwrap_or(0),
        );

        // Determine the starting act from the first score number
        let first_act = track.score_numbers.first().and_then(|sn| sn.act.clone());

        let mut segments: Vec<InterchangeSegment> = Vec::new();
        let mut current_act: Option<String> = None;

        for sn in &track.score_numbers {
            let seg_type = map_segment_type(&sn.content_type);

            // Detect act transition
            let act_changed = match (&sn.act, &current_act) {
                (Some(new_act), Some(cur_act)) => new_act != cur_act,
                (Some(_), None) => true,
                _ => false,
            };
            if let Some(ref act) = sn.act {
                current_act = Some(act.clone());
            }

            // Inject stage direction as a direction segment if present
            if let Some(ref dir) = sn.libretto_direction {
                // Use the start time of the first segment in this number, or 0
                let dir_start = sn.segments.first()
                    .and_then(|s| s.start)
                    .unwrap_or(0.0);

                segments.push(InterchangeSegment {
                    start: dir_start,
                    end: None,
                    segment_type: "direction".to_string(),
                    character: None,
                    text: None,
                    translation: None,
                    direction: Some(dir.clone()),
                    act: if act_changed { sn.act.clone() } else { None },
                    scene: Some(sn.title.clone()),
                    group: None,
                });
            }

            for (si, seg) in sn.segments.iter().enumerate() {
                let start = match seg.start {
                    Some(s) => s,
                    None => continue, // Skip untimed segments
                };

                // Set act/scene on the first segment of a new score number
                let is_first = si == 0 && sn.libretto_direction.is_none();

                segments.push(InterchangeSegment {
                    start,
                    end: None,
                    segment_type: seg_type.to_string(),
                    character: seg.character.clone(),
                    text: seg.text.clone(),
                    translation: seg.translation.clone(),
                    direction: None,
                    act: if is_first && act_changed { sn.act.clone() } else { None },
                    scene: if is_first { Some(sn.title.clone()) } else { None },
                    group: seg.group.clone(),
                });
            }
        }

        InterchangeTrack {
            track_id,
            title: track.track_title.clone(),
            disc_number: track.disc_number,
            track_number: track.track_number,
            duration_seconds: track.duration_seconds,
            act: first_act,
            scene: None,
            segments,
        }
    }).collect();

    InterchangeLibretto {
        version: "1.0-score".to_string(),
        opera,
        recording: Some(recording),
        tracks,
    }
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_for_score_match() {
        // Basic normalization
        assert_eq!(normalize_for_score_match("Cinque...dieci...venti"), "cinquedieciventi");
        // Hyphen removal (OCR syllable breaks)
        assert_eq!(normalize_for_score_match("Co-sa stai mi-su-rando"), "cosa stai misurando");
        // Newline collapse
        assert_eq!(normalize_for_score_match("Signori, di fuori\nson già"), "signori di fuori son gia");
        // Accent removal
        assert_eq!(normalize_for_score_match("perchè"), "perche");
    }

    #[test]
    fn test_compact_for_score_match() {
        // OCR syllable splits
        assert_eq!(compact_for_score_match("Cru del! perchè fi"), "crudelperche fi".replace(' ', ""));
        assert_eq!(compact_for_score_match("Ri co no sci"), "riconosci");
        assert_eq!(compact_for_score_match("Il ca pro e"), "ilcaproe");
    }

    #[test]
    fn test_text_prefix_match_basic() {
        assert!(text_prefix_match("Cinque...dieci...venti", "Cinque...dieci...venti"));
        assert!(text_prefix_match("Se a caso madama", "Se a"));
        assert!(!text_prefix_match("abc", "xyz"));
    }

    #[test]
    fn test_text_prefix_match_compact() {
        // OCR syllable splits should match via compact fallback
        assert!(text_prefix_match("Cru del! perchè fi", "Crudel! perchè finora"));
        assert!(text_prefix_match("Ri co no sci", "Riconosci in questo amplesso"));
        assert!(text_prefix_match("Il ca pro e", "Il capro e la capretta"));
    }

    #[test]
    fn test_text_prefix_match_newlines() {
        assert!(text_prefix_match(
            "Signori, di fuori\nson già i suonatori",
            "Signori, di fuori son già"
        ));
    }

    #[test]
    fn test_find_anchor_first_words() {
        let sn = ScoreNumber {
            id: 1,
            title: "DUETTO".to_string(),
            content_type: "number".to_string(),
            first_words: Some("Cinque...dieci...venti".to_string()),
            act: None, form: None, characters: vec![], pages: vec![],
            segments: vec![], libretto_title: None, libretto_act: None,
            libretto_characters: vec![], libretto_direction: None,
            libretto_form: None, libretto_number_ids: vec![],
        };
        let result = find_anchor_in_score_number(&sn, "Cinque...dieci...venti", false);
        assert_eq!(result.as_deref(), Some("first_words"));
    }

    #[test]
    fn test_find_anchor_title_quote() {
        let sn = ScoreNumber {
            id: 60,
            title: "ARIA N° 55. \"In quegli anni in cui val poco\" (Tenore.)".to_string(),
            content_type: "number".to_string(),
            first_words: Some("In que-gl'an-ni in cui".to_string()),
            act: None, form: None, characters: vec![], pages: vec![],
            segments: vec![], libretto_title: None, libretto_act: None,
            libretto_characters: vec![], libretto_direction: None,
            libretto_form: None, libretto_number_ids: vec![],
        };
        // first_words has OCR elision: "quegl'anni" vs "quegli anni" — doesn't match
        // but title quote has clean text that does match
        let result = find_anchor_in_score_number(&sn, "In quegli anni in cui val poco", false);
        assert_eq!(result.as_deref(), Some("title_quote"));
    }

    #[test]
    fn test_find_anchor_segment() {
        let sn = ScoreNumber {
            id: 37,
            title: "SCENA IX FIGARO".to_string(),
            content_type: "number".to_string(),
            first_words: None,
            act: None, form: None, characters: vec![], pages: vec![],
            segments: vec![
                ScoreSegment {
                    id: "no-15-finale-1".to_string(),
                    character: Some("FIGARO".to_string()),
                    text: Some("Eccellenza!".to_string()),
                    translation: None, group: None, start: None,
                },
                ScoreSegment {
                    id: "no-15-finale-66".to_string(),
                    character: Some("FIGARO".to_string()),
                    text: Some("Signori, di fuori\nson già i suonatori".to_string()),
                    translation: None, group: None, start: None,
                },
            ],
            libretto_title: None, libretto_act: None,
            libretto_characters: vec![], libretto_direction: None,
            libretto_form: None, libretto_number_ids: vec![],
        };
        // Without segment search: no match
        assert!(find_anchor_in_score_number(&sn, "Signori, di fuori son già", false).is_none());
        // With segment search: matches
        let result = find_anchor_in_score_number(&sn, "Signori, di fuori son già", true);
        assert!(result.is_some());
        assert!(result.unwrap().starts_with("segment:"));
    }

    #[test]
    fn test_estimate_timings_basic() {
        let mut output = ScoreMergedOutput {
            version: "1.0".to_string(),
            front_matter: ScoreFrontMatter {
                title: "Test".to_string(),
                total_pages: None,
                languages: None,
                pages: vec![],
            },
            recording: RecordingMetadata {
                conductor: None, orchestra: None, year: None, label: None, album_title: None, album_title_aliases: vec![],
            },
            discogs: None,
            contributors: vec![],
            tracks: vec![MergedTrack {
                track_title: "Track 1".to_string(),
                disc_number: Some(1),
                track_number: Some(1),
                duration_seconds: Some(100.0),
                characters: vec![],
                score_numbers: vec![ScoreNumber {
                    id: 1,
                    title: "ARIA".to_string(),
                    content_type: "number".to_string(),
                    first_words: None, act: None, form: None,
                    characters: vec![], pages: vec![],
                    segments: vec![
                        ScoreSegment {
                            id: "s1".to_string(),
                            character: Some("A".to_string()),
                            text: Some("one two three".to_string()), // 3 words
                            translation: None, group: None, start: None,
                        },
                        ScoreSegment {
                            id: "s2".to_string(),
                            character: Some("B".to_string()),
                            text: Some("four five six seven eight nine ten".to_string()), // 7 words
                            translation: None, group: None, start: None,
                        },
                    ],
                    libretto_title: None, libretto_act: None,
                    libretto_characters: vec![], libretto_direction: None,
                    libretto_form: None, libretto_number_ids: vec![],
                }],
            }],
        };

        let (stats, warnings) = estimate_score_merge_timings(&mut output);
        assert!(warnings.is_empty(), "warnings: {:?}", warnings);
        assert_eq!(stats.tracks_estimated, 1);
        assert_eq!(stats.segments_estimated, 2);

        let seg0 = &output.tracks[0].score_numbers[0].segments[0];
        let seg1 = &output.tracks[0].score_numbers[0].segments[1];
        // Weights: 3, 7 → total 10
        // seg0: 0/10 * 100 = 0.0
        assert_eq!(seg0.start, Some(0.0));
        // seg1: 3/10 * 100 = 30.0
        assert_eq!(seg1.start, Some(30.0));
    }

    #[test]
    fn test_estimate_timings_recitative_discount() {
        let mut output = ScoreMergedOutput {
            version: "1.0".to_string(),
            front_matter: ScoreFrontMatter {
                title: "Test".to_string(),
                total_pages: None,
                languages: None,
                pages: vec![],
            },
            recording: RecordingMetadata {
                conductor: None, orchestra: None, year: None, label: None, album_title: None, album_title_aliases: vec![],
            },
            discogs: None,
            contributors: vec![],
            tracks: vec![MergedTrack {
                track_title: "Track 1".to_string(),
                disc_number: Some(1),
                track_number: Some(1),
                duration_seconds: Some(100.0),
                characters: vec![],
                score_numbers: vec![
                    ScoreNumber {
                        id: 1,
                        title: "ARIA".to_string(),
                        content_type: "number".to_string(),
                        first_words: None, act: None, form: None,
                        characters: vec![], pages: vec![],
                        segments: vec![
                            ScoreSegment {
                                id: "aria-1".to_string(),
                                character: Some("A".to_string()),
                                text: Some("one two three four five six seven eight nine ten".to_string()), // 10 words
                                translation: None, group: None, start: None,
                            },
                        ],
                        libretto_title: None, libretto_act: None,
                        libretto_characters: vec![], libretto_direction: None,
                        libretto_form: None, libretto_number_ids: vec![],
                    },
                    ScoreNumber {
                        id: 2,
                        title: "RECIT".to_string(),
                        content_type: "recitative".to_string(),
                        first_words: None, act: None, form: None,
                        characters: vec![], pages: vec![],
                        segments: vec![
                            ScoreSegment {
                                id: "recit-1".to_string(),
                                character: Some("B".to_string()),
                                text: Some("one two three four five six seven eight nine ten".to_string()), // 10 words
                                translation: None, group: None, start: None,
                            },
                        ],
                        libretto_title: None, libretto_act: None,
                        libretto_characters: vec![], libretto_direction: None,
                        libretto_form: None, libretto_number_ids: vec![],
                    },
                ],
            }],
        };

        let (stats, warnings) = estimate_score_merge_timings(&mut output);
        assert!(warnings.is_empty());
        assert_eq!(stats.segments_estimated, 2);

        let aria_start = output.tracks[0].score_numbers[0].segments[0].start.unwrap();
        let recit_start = output.tracks[0].score_numbers[1].segments[0].start.unwrap();

        // Weights: aria=10*1.0=10, recit=10*0.5=5 → total 15
        // aria: 0/15 * 100 = 0.0
        assert_eq!(aria_start, 0.0);
        // recit: 10/15 * 100 = 66.667
        assert!((recit_start - 66.667).abs() < 0.01, "recit_start={}", recit_start);
    }

    #[test]
    fn test_estimate_timings_direction_zero_weight() {
        // A direction segment (no text) should get weight 0, sharing the
        // start time of whatever comes next.
        let mut output = ScoreMergedOutput {
            version: "1.0".to_string(),
            front_matter: ScoreFrontMatter {
                title: "Test".to_string(),
                total_pages: None, languages: None, pages: vec![],
            },
            recording: RecordingMetadata {
                conductor: None, orchestra: None, year: None, label: None, album_title: None, album_title_aliases: vec![],
            },
            discogs: None,
            contributors: vec![],
            tracks: vec![MergedTrack {
                track_title: "Track 1".to_string(),
                disc_number: Some(1),
                track_number: Some(1),
                duration_seconds: Some(100.0),
                characters: vec![],
                score_numbers: vec![ScoreNumber {
                    id: 1,
                    title: "ARIA".to_string(),
                    content_type: "number".to_string(),
                    first_words: None, act: None, form: None,
                    characters: vec![], pages: vec![],
                    segments: vec![
                        ScoreSegment {
                            id: "dir".to_string(),
                            character: None,
                            text: None, // direction — no text
                            translation: None, group: None, start: None,
                        },
                        ScoreSegment {
                            id: "s1".to_string(),
                            character: Some("A".to_string()),
                            text: Some("one two three four five".to_string()), // 5 words
                            translation: None, group: None, start: None,
                        },
                        ScoreSegment {
                            id: "s2".to_string(),
                            character: Some("B".to_string()),
                            text: Some("six seven eight nine ten".to_string()), // 5 words
                            translation: None, group: None, start: None,
                        },
                    ],
                    libretto_title: None, libretto_act: None,
                    libretto_characters: vec![], libretto_direction: None,
                    libretto_form: None, libretto_number_ids: vec![],
                }],
            }],
        };

        let (stats, warnings) = estimate_score_merge_timings(&mut output);
        assert!(warnings.is_empty());
        assert_eq!(stats.segments_estimated, 3);

        let segs = &output.tracks[0].score_numbers[0].segments;
        // Direction: weight 0, starts at 0
        assert_eq!(segs[0].start, Some(0.0));
        // s1: weight 5, starts at 0 (direction had 0 weight)
        assert_eq!(segs[1].start, Some(0.0));
        // s2: weight 5, starts at 5/10 * 100 = 50.0
        assert_eq!(segs[2].start, Some(50.0));
    }

    #[test]
    fn test_estimate_timings_group_collapsing() {
        // Three segments with group "g1" should be collapsed:
        // weight = max(3, 7, 5) = 7, and all three get the same start.
        let mut output = ScoreMergedOutput {
            version: "1.0".to_string(),
            front_matter: ScoreFrontMatter {
                title: "Test".to_string(),
                total_pages: None, languages: None, pages: vec![],
            },
            recording: RecordingMetadata {
                conductor: None, orchestra: None, year: None, label: None, album_title: None, album_title_aliases: vec![],
            },
            discogs: None,
            contributors: vec![],
            tracks: vec![MergedTrack {
                track_title: "Track 1".to_string(),
                disc_number: Some(1),
                track_number: Some(1),
                duration_seconds: Some(100.0),
                characters: vec![],
                score_numbers: vec![ScoreNumber {
                    id: 1,
                    title: "TRIO".to_string(),
                    content_type: "number".to_string(),
                    first_words: None, act: None, form: None,
                    characters: vec![], pages: vec![],
                    segments: vec![
                        ScoreSegment {
                            id: "s1".to_string(),
                            character: Some("A".to_string()),
                            text: Some("one two three".to_string()), // 3 words
                            translation: None, group: None, start: None,
                        },
                        ScoreSegment {
                            id: "g1a".to_string(),
                            character: Some("A".to_string()),
                            text: Some("aaa bbb ccc".to_string()), // 3 words
                            translation: None, group: Some("g1".to_string()), start: None,
                        },
                        ScoreSegment {
                            id: "g1b".to_string(),
                            character: Some("B".to_string()),
                            text: Some("ddd eee fff ggg hhh iii jjj".to_string()), // 7 words
                            translation: None, group: Some("g1".to_string()), start: None,
                        },
                        ScoreSegment {
                            id: "g1c".to_string(),
                            character: Some("C".to_string()),
                            text: Some("kkk lll mmm nnn ooo".to_string()), // 5 words
                            translation: None, group: Some("g1".to_string()), start: None,
                        },
                        ScoreSegment {
                            id: "s5".to_string(),
                            character: Some("A".to_string()),
                            text: Some("one two three four five six seven eight nine ten".to_string()), // 10 words
                            translation: None, group: None, start: None,
                        },
                    ],
                    libretto_title: None, libretto_act: None,
                    libretto_characters: vec![], libretto_direction: None,
                    libretto_form: None, libretto_number_ids: vec![],
                }],
            }],
        };

        let (stats, warnings) = estimate_score_merge_timings(&mut output);
        assert!(warnings.is_empty());
        assert_eq!(stats.segments_estimated, 5);

        let segs = &output.tracks[0].score_numbers[0].segments;
        // Units: s1(3) + group_g1(max(3,7,5)=7) + s5(10) → total=20
        // s1: 0/20 * 100 = 0.0
        assert_eq!(segs[0].start, Some(0.0));
        // g1a, g1b, g1c: all at 3/20 * 100 = 15.0
        assert_eq!(segs[1].start, Some(15.0));
        assert_eq!(segs[2].start, Some(15.0));
        assert_eq!(segs[3].start, Some(15.0));
        // s5: (3+7)/20 * 100 = 50.0
        assert_eq!(segs[4].start, Some(50.0));
    }
}
