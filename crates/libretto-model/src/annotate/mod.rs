// Score-derived annotation for base librettos.
//
// An annotation file captures information extracted from a vocal score
// (group tags for simultaneous singing, tempo markings, number type
// corrections) and can be applied to a base libretto to enrich it.
//
// The `generate` submodule uses an LLM (Claude) to read score page images
// and produce annotations automatically.

use serde::{Deserialize, Serialize};

use crate::base_libretto::{BaseLibretto, NumberType};

/// A complete set of score-derived annotations for a base libretto.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreAnnotation {
    /// Information about the source score.
    pub source: AnnotationSource,
    /// Per-number annotations. Each entry's `number_id` must match
    /// a `MusicalNumber.id` in the target base libretto.
    pub numbers: Vec<NumberAnnotation>,
}

/// Metadata about the score used to produce annotations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnnotationSource {
    /// Human-readable title (e.g., "Ricordi Le nozze di Figaro").
    pub score_title: String,
    /// External identifier (e.g., "IMSLP873093").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score_id: Option<String>,
}

/// Annotations for a single musical number.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NumberAnnotation {
    /// Must match a `MusicalNumber.id` in the base libretto.
    pub number_id: String,
    /// Tempo marking from the score (e.g., "Allegro").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tempo_marking: Option<String>,
    /// Override number type if different from what the parser produced.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub number_type: Option<NumberType>,
    /// Groups of segments that are sung simultaneously.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub groups: Vec<GroupAnnotation>,
}

/// A group of segments sung simultaneously within a number.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupAnnotation {
    /// Group tag (e.g., "a", "b"). Segments with the same group within
    /// a number overlap in time and should be displayed together.
    pub group: String,
    /// Segment IDs belonging to this group.
    pub segment_ids: Vec<String>,
}

/// Result of applying annotations.
#[derive(Debug)]
pub struct ApplyResult {
    /// Number of tempo markings set.
    pub tempo_markings_set: usize,
    /// Number of number types overridden.
    pub number_types_set: usize,
    /// Number of group tags applied to segments.
    pub groups_applied: usize,
    /// Warnings (e.g., unknown number_id or segment_id).
    pub warnings: Vec<String>,
}

/// Apply a set of score annotations to a base libretto, mutating it in place.
pub fn apply(base: &mut BaseLibretto, annotations: &ScoreAnnotation) -> ApplyResult {
    let mut result = ApplyResult {
        tempo_markings_set: 0,
        number_types_set: 0,
        groups_applied: 0,
        warnings: Vec::new(),
    };

    for ann in &annotations.numbers {
        let number = match base.numbers.iter_mut().find(|n| n.id == ann.number_id) {
            Some(n) => n,
            None => {
                result.warnings.push(format!(
                    "Annotation references unknown number_id '{}'", ann.number_id
                ));
                continue;
            }
        };

        // Apply tempo marking
        if let Some(ref tempo) = ann.tempo_marking {
            number.tempo_marking = Some(tempo.clone());
            result.tempo_markings_set += 1;
        }

        // Apply number type override
        if let Some(ref nt) = ann.number_type {
            number.number_type = nt.clone();
            result.number_types_set += 1;
        }

        // Apply group tags
        for group_ann in &ann.groups {
            for seg_id in &group_ann.segment_ids {
                match number.segments.iter_mut().find(|s| s.id == *seg_id) {
                    Some(seg) => {
                        seg.group = Some(group_ann.group.clone());
                        result.groups_applied += 1;
                    }
                    None => {
                        result.warnings.push(format!(
                            "Number '{}': group '{}' references unknown segment_id '{}'",
                            ann.number_id, group_ann.group, seg_id
                        ));
                    }
                }
            }
        }
    }

    result
}

/// A mapping of musical number IDs to their page ranges in a score.
///
/// Produced by the `map` step and consumed by `generate` to annotate
/// all numbers automatically.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreMap {
    /// Total pages in the score.
    pub total_pages: u32,
    /// Per-number page mappings.
    pub numbers: Vec<ScoreMapEntry>,
}

/// A single entry in a score map.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreMapEntry {
    /// Musical number ID from the base libretto (e.g., "no-7-terzetto").
    pub number_id: String,
    /// Label as it appears in the score (e.g., "N° 15 Terzetto").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score_label: Option<String>,
    /// 1-indexed page numbers in the score.
    pub pages: Vec<u32>,
}

/// Intermediate page descriptions from the LLM (Phase 1 of map).
///
/// Claude describes what it sees on each page without any reference to
/// our base libretto. The matching happens in Phase 2 (Rust code).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageDescriptions {
    /// Total pages in the score.
    pub total_pages: u32,
    /// Per-page descriptions, sorted by page number.
    pub pages: Vec<PageDescription>,
}

/// What the LLM observed on a single score page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageDescription {
    /// 1-indexed page number.
    pub page: u32,
    /// Content type observed on the page.
    pub content_type: PageContentType,
    /// Title/heading printed on the page, if any (e.g., "N° 15 Terzetto").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Character names visible on vocal staves.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub characters: Vec<String>,
    /// First few words of sung text visible on the page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_words: Option<String>,
    /// Last few words of sung text visible on the page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_words: Option<String>,
    /// Whether this page is a continuation of the piece on the previous page
    /// (no new title/heading appears).
    #[serde(default)]
    pub continuation: bool,
}

/// The type of musical content on a score page.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PageContentType {
    /// Overture (orchestral, no voices).
    Overture,
    /// A musical number (aria, duet, ensemble, etc.).
    Number,
    /// Recitative passage.
    Recitative,
    /// Title pages, table of contents, etc.
    FrontMatter,
    /// Blank or unrecognizable page.
    Other,
}

/// Extract all pages from a score PDF as PNG images.
///
/// Returns the number of pages extracted.
pub fn ingest_score(
    pdf_path: &std::path::Path,
    output_dir: &std::path::Path,
    dpi: u32,
) -> Result<u32, String> {
    std::fs::create_dir_all(output_dir)
        .map_err(|e| format!("Failed to create output dir: {e}"))?;

    let doc = mupdf::Document::open(pdf_path.to_str().unwrap_or(""))
        .map_err(|e| format!("Failed to open PDF: {e}"))?;

    let page_count = doc.page_count()
        .map_err(|e| format!("Failed to get page count: {e}"))?;

    let scale = dpi as f32 / 72.0;
    let matrix = mupdf::Matrix::new_scale(scale, scale);

    for i in 0..page_count {
        let page_num = (i + 1) as u32;
        let page = doc.load_page(i)
            .map_err(|e| format!("Failed to load page {page_num}: {e}"))?;

        let pixmap = page.to_pixmap(&matrix, &mupdf::Colorspace::device_rgb(), false, false)
            .map_err(|e| format!("Failed to render page {page_num}: {e}"))?;

        let out_path = output_dir.join(format!("p{page_num:03}.png"));
        pixmap.save_as(out_path.to_str().unwrap_or(""), mupdf::pixmap::ImageFormat::PNG)
            .map_err(|e| format!("Failed to save page {page_num}: {e}"))?;

        if page_num % 50 == 0 {
            tracing::info!(page = page_num, total = page_count, "Ingesting pages...");
        }
    }

    Ok(page_count as u32)
}

pub mod generate;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base_libretto::*;

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
            tempo_marking: None,
            segments: vec![
                Segment {
                    id: "no-1-001".to_string(),
                    segment_type: SegmentType::Sung,
                    character: Some("A".to_string()),
                    text: Some("First line".to_string()),
                    translation: None,
                    direction: None,
                    group: None,
                },
                Segment {
                    id: "no-1-002".to_string(),
                    segment_type: SegmentType::Sung,
                    character: Some("B".to_string()),
                    text: Some("Second line".to_string()),
                    translation: None,
                    direction: None,
                    group: None,
                },
                Segment {
                    id: "no-1-003".to_string(),
                    segment_type: SegmentType::Sung,
                    character: Some("A".to_string()),
                    text: Some("Third line".to_string()),
                    translation: None,
                    direction: None,
                    group: None,
                },
            ],
        });
        lib
    }

    #[test]
    fn test_apply_groups() {
        let mut base = test_base();
        let ann = ScoreAnnotation {
            source: AnnotationSource {
                score_title: "Test".to_string(),
                score_id: None,
            },
            numbers: vec![NumberAnnotation {
                number_id: "no-1".to_string(),
                tempo_marking: Some("Allegro".to_string()),
                number_type: None,
                groups: vec![GroupAnnotation {
                    group: "a".to_string(),
                    segment_ids: vec!["no-1-001".to_string(), "no-1-002".to_string()],
                }],
            }],
        };

        let result = apply(&mut base, &ann);

        assert_eq!(result.tempo_markings_set, 1);
        assert_eq!(result.groups_applied, 2);
        assert_eq!(result.warnings.len(), 0);
        assert_eq!(base.numbers[0].tempo_marking.as_deref(), Some("Allegro"));
        assert_eq!(base.numbers[0].segments[0].group.as_deref(), Some("a"));
        assert_eq!(base.numbers[0].segments[1].group.as_deref(), Some("a"));
        assert_eq!(base.numbers[0].segments[2].group, None);
    }

    #[test]
    fn test_apply_number_type_override() {
        let mut base = test_base();
        let ann = ScoreAnnotation {
            source: AnnotationSource {
                score_title: "Test".to_string(),
                score_id: None,
            },
            numbers: vec![NumberAnnotation {
                number_id: "no-1".to_string(),
                tempo_marking: None,
                number_type: Some(NumberType::Duet),
                groups: vec![],
            }],
        };

        let result = apply(&mut base, &ann);

        assert_eq!(result.number_types_set, 1);
        assert_eq!(base.numbers[0].number_type, NumberType::Duet);
    }

    #[test]
    fn test_apply_unknown_ids_warn() {
        let mut base = test_base();
        let ann = ScoreAnnotation {
            source: AnnotationSource {
                score_title: "Test".to_string(),
                score_id: None,
            },
            numbers: vec![
                NumberAnnotation {
                    number_id: "no-99".to_string(),
                    tempo_marking: None,
                    number_type: None,
                    groups: vec![],
                },
                NumberAnnotation {
                    number_id: "no-1".to_string(),
                    tempo_marking: None,
                    number_type: None,
                    groups: vec![GroupAnnotation {
                        group: "a".to_string(),
                        segment_ids: vec!["no-1-999".to_string()],
                    }],
                },
            ],
        };

        let result = apply(&mut base, &ann);

        assert_eq!(result.warnings.len(), 2);
        assert!(result.warnings[0].contains("no-99"));
        assert!(result.warnings[1].contains("no-1-999"));
    }

    #[test]
    fn test_roundtrip_json() {
        let ann = ScoreAnnotation {
            source: AnnotationSource {
                score_title: "Ricordi Le nozze di Figaro".to_string(),
                score_id: Some("IMSLP873093".to_string()),
            },
            numbers: vec![NumberAnnotation {
                number_id: "no-1-duettino".to_string(),
                tempo_marking: Some("Allegro".to_string()),
                number_type: None,
                groups: vec![GroupAnnotation {
                    group: "a".to_string(),
                    segment_ids: vec![
                        "no-1-duettino-001".to_string(),
                        "no-1-duettino-002".to_string(),
                    ],
                }],
            }],
        };

        let json = serde_json::to_string_pretty(&ann).unwrap();
        let parsed: ScoreAnnotation = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.source.score_title, "Ricordi Le nozze di Figaro");
        assert_eq!(parsed.numbers[0].groups[0].segment_ids.len(), 2);
    }
}
