// Merge score.json + libretto.json → merged_score_libretto.json
//
// Aligns libretto segments to score numbers using first_words and
// title-quoted-text matching. The output preserves the full score
// structure with libretto text segments attached to each number.

use serde::{Deserialize, Serialize};

use crate::annotate::PageContentType;
use crate::bilingual::{Libretto, LibrettoLanguages, LibrettoSegment};
use crate::score::{Score, ScorePage, ScoreNumber};

// ---------------------------------------------------------------------------
// Output types
// ---------------------------------------------------------------------------

/// Merged score+libretto document.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergedScore {
    pub front_matter: MergedFrontMatter,
    pub numbers: Vec<MergedNumber>,
}

/// Front matter combining score metadata and libretto languages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergedFrontMatter {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    pub total_pages: u32,
    pub languages: LibrettoLanguages,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pages: Vec<ScorePage>,
}

/// A score number enriched with libretto segments and metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergedNumber {
    pub id: u32,
    pub content_type: PageContentType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub act: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub characters: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_words: Option<String>,
    pub pages: Vec<ScorePage>,

    // Libretto data
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub libretto_number_ids: Vec<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub libretto_act: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub libretto_form: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub libretto_title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub libretto_direction: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub libretto_characters: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub segments: Vec<MergedSegment>,
}

/// A libretto segment attached to a score number.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MergedSegment {
    pub id: String,
    pub character: String,
    pub text: String,
    pub translation: String,
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct FlatSegment {
    seg: LibrettoSegment,
    meta: LibNumMeta,
}

#[derive(Clone)]
struct LibNumMeta {
    id: u32,
    title: String,
    form: String,
    act: Option<String>,
    direction: Option<String>,
    characters: Vec<String>,
}

/// Build result with optional diagnostics.
pub struct BuildBaseResult {
    pub merged: MergedScore,
    pub warnings: Vec<String>,
}

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Merge a score and libretto into a single document.
///
/// Each score number is matched to libretto segments by comparing
/// first_words (or title-quoted text) against segment text. Segments
/// between consecutive anchors are assigned to the earlier anchor's
/// score number.
pub fn build_base(score: &Score, libretto: &Libretto) -> BuildBaseResult {
    let mut warnings: Vec<String> = Vec::new();

    // 1. Flatten libretto segments with metadata
    let flat: Vec<FlatSegment> = flatten_libretto(libretto);

    // 2. Find anchor points: score_number_index → segment_index
    let anchors = find_anchors(&score.numbers, &flat, &mut warnings);

    // 3. Assign segment ranges from anchors
    let ranges = assign_ranges(&anchors, flat.len());

    // 4. Build output
    let numbers: Vec<MergedNumber> = score
        .numbers
        .iter()
        .enumerate()
        .map(|(i, sn)| build_merged_number(sn, &flat[ranges[i].0..ranges[i].1]))
        .collect();

    let total_assigned: usize = numbers.iter().map(|n| n.segments.len()).sum();
    let unmatched = flat.len() - total_assigned;
    if unmatched > 0 {
        warnings.push(format!("{unmatched} libretto segments were not assigned to any score number"));
    }

    BuildBaseResult {
        merged: MergedScore {
            front_matter: MergedFrontMatter {
                title: score.front_matter.title.clone(),
                total_pages: score.front_matter.total_pages,
                languages: libretto.front_matter.languages.clone(),
                pages: score.front_matter.pages.clone(),
            },
            numbers,
        },
        warnings,
    }
}

// ---------------------------------------------------------------------------
// Flatten
// ---------------------------------------------------------------------------

fn flatten_libretto(libretto: &Libretto) -> Vec<FlatSegment> {
    libretto
        .numbers
        .iter()
        .flat_map(|n| {
            let meta = LibNumMeta {
                id: n.id,
                title: n.title.clone(),
                form: n.form.clone(),
                act: n.act.clone(),
                direction: n.direction.clone(),
                characters: n.characters.clone(),
            };
            n.segments.iter().map(move |s| FlatSegment {
                seg: s.clone(),
                meta: meta.clone(),
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Anchor finding
// ---------------------------------------------------------------------------

fn find_anchors(
    score_numbers: &[ScoreNumber],
    flat: &[FlatSegment],
    warnings: &mut Vec<String>,
) -> Vec<Option<usize>> {
    let mut anchors: Vec<Option<usize>> = vec![None; score_numbers.len()];
    let mut cursor: usize = 0;

    for (i, sn) in score_numbers.iter().enumerate() {
        let title = sn.title.as_deref().unwrap_or("");

        // Try both strategies; take the earliest (smallest index) match.
        // This handles "RECITATIVO ED ARIA" numbers correctly: first_words
        // matches the recitative start, quoted text matches the aria,
        // and we pick the earlier one.
        let mut best: Option<usize> = None;

        // Strategy 1: quoted text from title (clean — 20% error tolerance)
        if let Some(quoted) = extract_quoted(title) {
            let nq = normalize(quoted);
            if nq.len() >= 4 {
                if let Some(idx) = search_match(&nq, flat, cursor, 300, 20) {
                    best = Some(idx);
                }
            }
        }

        // Strategy 2: first_words (OCR artifacts — 20% error tolerance)
        if let Some(fw) = &sn.first_words {
            let nfw = normalize(fw);
            if nfw.len() >= 4 {
                if let Some(idx) = search_match(&nfw, flat, cursor, 300, 20) {
                    best = match best {
                        Some(prev) => Some(prev.min(idx)),
                        None => Some(idx),
                    };
                }
            }
        }

        if let Some(idx) = best {
            anchors[i] = Some(idx);
            cursor = idx;
            continue;
        }

        // No match
        if sn.first_words.is_some() || extract_quoted(title).is_some() {
            warnings.push(format!(
                "Score {}: no segment match for {:?} / first_words={:?}",
                sn.id,
                sn.title.as_deref().unwrap_or("-"),
                sn.first_words.as_deref().unwrap_or("-"),
            ));
        }
    }

    anchors
}

/// Search for a matching segment in the flat list.
///
/// `max_error_pct` is the maximum edit distance as a percentage of
/// query length (e.g., 25 means up to 25% errors allowed).
fn search_match(
    query_norm: &str,
    flat: &[FlatSegment],
    from: usize,
    window: usize,
    max_error_pct: usize,
) -> Option<usize> {
    let end = (from + window).min(flat.len());
    for i in from..end {
        let ns = normalize(&flat[i].seg.text);
        if is_prefix_edit_match(query_norm, &ns, max_error_pct) {
            return Some(i);
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Text matching
// ---------------------------------------------------------------------------

/// Normalize a string for matching: lowercase, strip accents, keep only alphanumeric.
fn normalize(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(|c| c.to_lowercase())
        .map(strip_accent)
        .collect()
}

/// Map accented characters to their ASCII base form.
fn strip_accent(c: char) -> char {
    match c {
        'à' | 'á' | 'â' | 'ã' | 'ä' => 'a',
        'è' | 'é' | 'ê' | 'ë' => 'e',
        'ì' | 'í' | 'î' | 'ï' => 'i',
        'ò' | 'ó' | 'ô' | 'õ' | 'ö' => 'o',
        'ù' | 'ú' | 'û' | 'ü' => 'u',
        'ñ' => 'n',
        'ç' => 'c',
        _ => c,
    }
}

/// Extract the first quoted substring from a title.
/// Handles both ASCII `"` and Unicode `\u{201c}`/`\u{201d}` quotes.
fn extract_quoted(title: &str) -> Option<&str> {
    // Try ASCII quotes first
    if let Some(result) = extract_between(title, '"', '"') {
        return Some(result);
    }
    // Try Unicode left/right quotes
    if let Some(start) = title.find('\u{201c}') {
        let after = start + '\u{201c}'.len_utf8();
        if let Some(end_off) = title[after..].find('\u{201d}') {
            let result = &title[after..after + end_off];
            if result.len() >= 4 {
                return Some(result);
            }
        }
    }
    None
}

fn extract_between(s: &str, open: char, close: char) -> Option<&str> {
    let start = s.find(open)? + open.len_utf8();
    let end = start + s[start..].find(close)?;
    let inner = &s[start..end];
    if inner.len() >= 4 {
        Some(inner)
    } else {
        None
    }
}

/// Prefix match using edit distance.
///
/// The first 3 characters must match exactly (quick filter).
/// Then computes the edit distance between the query and the best-matching
/// prefix of the text. Accepts if distance ≤ `max_error_pct`% of query length.
///
/// Edit distance correctly handles OCR insertions/deletions (e.g., "ormai" →
/// "o mai" dropping 'r') and character substitutions (e.g., "vinta" vs "vinto").
fn is_prefix_edit_match(query_norm: &str, text_norm: &str, max_error_pct: usize) -> bool {
    let qchars: Vec<char> = query_norm.chars().take(20).collect();
    if qchars.len() < 4 {
        return false;
    }
    let tchars: Vec<char> = text_norm.chars().collect();

    // Quick filter: first 3 characters must match exactly
    if tchars.len() < 3 {
        return false;
    }
    for i in 0..3 {
        if qchars[i] != tchars[i] {
            return false;
        }
    }

    let max_errors = (qchars.len() * max_error_pct + 99) / 100;
    let m = qchars.len();
    let n = tchars.len().min(m + max_errors + 3);

    // Standard Levenshtein DP; take min of last row for best prefix match.
    let mut prev: Vec<usize> = (0..=n).collect();
    let mut curr = vec![0usize; n + 1];

    for i in 1..=m {
        curr[0] = i;
        for j in 1..=n {
            let cost = if qchars[i - 1] == tchars[j - 1] { 0 } else { 1 };
            curr[j] = (prev[j] + 1)
                .min(curr[j - 1] + 1)
                .min(prev[j - 1] + cost);
        }
        std::mem::swap(&mut prev, &mut curr);
    }

    // Minimum edit distance matching query against any prefix of text
    *prev.iter().min().unwrap() <= max_errors
}

// ---------------------------------------------------------------------------
// Range assignment
// ---------------------------------------------------------------------------

/// Convert anchors into (start, end) ranges for each score number.
///
/// Each anchored score number gets segments from its anchor to the next
/// anchor. Non-anchored numbers get empty ranges.
fn assign_ranges(anchors: &[Option<usize>], total_segs: usize) -> Vec<(usize, usize)> {
    let mut ranges: Vec<(usize, usize)> = vec![(0, 0); anchors.len()];

    // Collect anchored positions
    let anchored: Vec<(usize, usize)> = anchors
        .iter()
        .enumerate()
        .filter_map(|(i, a)| a.map(|s| (i, s)))
        .collect();

    for (ai, &(score_idx, seg_start)) in anchored.iter().enumerate() {
        let seg_end = if ai + 1 < anchored.len() {
            anchored[ai + 1].1
        } else {
            total_segs
        };
        ranges[score_idx] = (seg_start, seg_end);
    }

    ranges
}

// ---------------------------------------------------------------------------
// Build output
// ---------------------------------------------------------------------------

fn build_merged_number(sn: &ScoreNumber, segs: &[FlatSegment]) -> MergedNumber {
    // Collect libretto metadata from contributing numbers
    let mut lib_ids: Vec<u32> = Vec::new();
    let mut lib_acts: Vec<String> = Vec::new();
    let mut lib_forms: Vec<String> = Vec::new();
    let mut lib_titles: Vec<String> = Vec::new();
    let mut lib_directions: Vec<String> = Vec::new();
    let mut lib_chars: Vec<String> = Vec::new();

    for fs in segs {
        if !lib_ids.contains(&fs.meta.id) {
            lib_ids.push(fs.meta.id);
            if !lib_titles.contains(&fs.meta.title) {
                lib_titles.push(fs.meta.title.clone());
            }
            if !lib_forms.contains(&fs.meta.form) {
                lib_forms.push(fs.meta.form.clone());
            }
            if let Some(ref act) = fs.meta.act {
                if !lib_acts.contains(act) {
                    lib_acts.push(act.clone());
                }
            }
            if let Some(ref dir) = fs.meta.direction {
                if !lib_directions.contains(dir) {
                    lib_directions.push(dir.clone());
                }
            }
            for c in &fs.meta.characters {
                if !lib_chars.contains(c) {
                    lib_chars.push(c.clone());
                }
            }
        }
    }

    let segments: Vec<MergedSegment> = segs
        .iter()
        .map(|fs| MergedSegment {
            id: fs.seg.id.clone(),
            character: fs.seg.character.clone(),
            text: fs.seg.text.clone(),
            translation: fs.seg.translation.clone(),
        })
        .collect();

    MergedNumber {
        id: sn.id,
        content_type: sn.content_type.clone(),
        form: sn.form.clone(),
        title: sn.title.clone(),
        act: sn.act.clone(),
        characters: sn.characters.clone(),
        first_words: sn.first_words.clone(),
        pages: sn.pages.clone(),
        libretto_number_ids: lib_ids,
        libretto_act: lib_acts.first().cloned(),
        libretto_form: match lib_forms.len() {
            0 => None,
            1 => lib_forms.into_iter().next(),
            _ => Some(lib_forms.join(", ")),
        },
        libretto_title: match lib_titles.len() {
            0 => None,
            1 => lib_titles.into_iter().next(),
            _ => Some(lib_titles.join("; ")),
        },
        libretto_direction: match lib_directions.len() {
            0 => None,
            1 => lib_directions.into_iter().next(),
            _ => Some(lib_directions.join("\n")),
        },
        libretto_characters: lib_chars,
        segments,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize() {
        assert_eq!(normalize("Cinque...dieci..."), "cinquedieci");
        assert_eq!(normalize("E sci o mai garzon"), "esciomaigarzon");
        assert_eq!(normalize("Esci ormai, garzon malnato"), "esciormaigarzonmalnato");
        // Accents stripped: è → e, ò → o
        assert_eq!(normalize("Tutto è disposto"), "tuttoedisposto");
        assert_eq!(normalize("perchè"), "perche");
    }

    #[test]
    fn test_extract_quoted() {
        assert_eq!(
            extract_quoted(r#"N° 34. FINALE "Esci omai, garzon""#),
            Some("Esci omai, garzon")
        );
        assert_eq!(extract_quoted("RECITATIVO"), None);
        assert_eq!(extract_quoted(r#"ARIA "ab""#), None); // too short
    }

    #[test]
    fn test_prefix_edit_match() {
        // Exact prefix
        assert!(is_prefix_edit_match("cinquedieci", "cinquedieciventitrenta", 30));
        // OCR drops 'r': "esciomaigarzon" vs "esciormaigarzonmalnato"
        assert!(is_prefix_edit_match("esciomaigarzon", "esciormaigarzonmalnato", 30));
        // Too short
        assert!(!is_prefix_edit_match("abc", "abcdef", 30));
        // No match: first 3 chars differ
        assert!(!is_prefix_edit_match("zzzzzzzzz", "abcdefghijklm", 30));
        // Rejects false positive: common prefix "equesto" but divergent middle
        assert!(!is_prefix_edit_match(
            &normalize("È questo il sito"),
            &normalize("E questo, signor scaltro"),
            20,
        ));
    }

    #[test]
    fn test_edit_match_ocr_artifacts() {
        // Accented char difference: perchè vs perché
        assert!(is_prefix_edit_match(
            &normalize("Cru del! perchè fi"),
            &normalize("Crudel! Perché finora"),
            30,
        ));
        // Single-char substitution: vinta vs vinto
        assert!(is_prefix_edit_match(
            &normalize("Hai già vinta la causa?"),
            &normalize("Hai già vinto la causa! Cosa sento!"),
            30,
        ));
        // OCR spaces within words
        assert!(is_prefix_edit_match(
            &normalize("Ri co no sci"),
            &normalize("Riconosci in quest'amplesso"),
            30,
        ));
    }
}
