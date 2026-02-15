// Italian/English parallel alignment.
//
// Walks Italian and English parsed structures in parallel, pairing
// segments. If a `bilingual.json` from murashev.com is available,
// uses its pre-aligned pairs for higher-confidence matching.

use libretto_acquire::types::{AcquiredLibretto, ContentElement};
use libretto_model::base_libretto::Segment;

use crate::cast;
use crate::structure;
use crate::segments;

/// Align two sets of segments by pairing translations.
///
/// Given segments from the original language and segments from the
/// translation, match them by number ID and sequence position,
/// then copy translation text into the original segments.
pub fn align_segments(
    original: &mut Vec<Segment>,
    translation: &[Segment],
) {
    // Build a lookup: (number_id_prefix, seq) → translation text
    // Segment IDs are like "no-1-duettino-001" — the prefix is everything
    // before the last "-NNN".
    for orig_seg in original.iter_mut() {
        // Find the matching translation segment by ID
        if let Some(trans_seg) = translation.iter().find(|t| t.id == orig_seg.id) {
            orig_seg.translation = trans_seg.text.clone();
        }
    }
}

/// Parse a bilingual acquisition into aligned segments.
///
/// The bilingual JSON has pre-aligned rows where lang1 and lang2 elements
/// correspond 1:1. We run the full pipeline on each language column
/// independently, then align by segment ID.
///
/// Returns `(original_language_segments_per_number, translation_language)`.
pub fn parse_bilingual(libretto: &AcquiredLibretto) -> (Vec<ContentElement>, Vec<ContentElement>) {
    // Flatten all rows into a single element sequence per language
    let lang1_elements: Vec<ContentElement> = libretto.rows.iter()
        .flat_map(|row| row.lang1_elements.clone())
        .collect();
    let lang2_elements: Vec<ContentElement> = libretto.rows.iter()
        .flat_map(|row| row.lang2_elements.clone())
        .collect();

    (lang1_elements, lang2_elements)
}

/// Run the full parse pipeline on a single element sequence:
/// cast extraction → structure splitting → segment splitting.
///
/// Returns the segments for all numbers, in order.
pub fn pipeline(elements: &[ContentElement]) -> PipelineResult {
    let cast_result = cast::extract_cast(elements);
    let remaining = &elements[cast_result.end_index..];
    let numbers = structure::split_into_numbers(remaining);

    let mut all_segments = Vec::new();
    let mut number_metadata = Vec::new();

    for number in &numbers {
        let segs = segments::split_segments(number);
        number_metadata.push(NumberMeta {
            id: number.id.clone(),
            label: number.label.clone(),
            number_type: number.number_type.clone(),
            act: number.act.clone(),
            scene: number.scene.clone(),
            segment_count: segs.len(),
        });
        all_segments.extend(segs);
    }

    PipelineResult {
        cast: cast_result.members,
        numbers: number_metadata,
        segments: all_segments,
    }
}

/// Result of running the full parse pipeline on one language.
pub struct PipelineResult {
    pub cast: Vec<libretto_model::base_libretto::CastMember>,
    pub numbers: Vec<NumberMeta>,
    pub segments: Vec<Segment>,
}

/// Metadata about a musical number (without the segments themselves).
#[derive(Debug, Clone)]
pub struct NumberMeta {
    pub id: String,
    pub label: String,
    pub number_type: libretto_model::base_libretto::NumberType,
    pub act: String,
    pub scene: Option<String>,
    pub segment_count: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use libretto_model::base_libretto::SegmentType;

    #[test]
    fn test_align_segments() {
        let mut original = vec![
            Segment {
                id: "no-1-duettino-001".to_string(),
                segment_type: SegmentType::Sung,
                character: Some("FIGARO".to_string()),
                text: Some("Cinque... dieci...".to_string()),
                translation: None,
                direction: None,
                group: None,
            },
            Segment {
                id: "no-1-duettino-002".to_string(),
                segment_type: SegmentType::Sung,
                character: Some("SUSANNA".to_string()),
                text: Some("Ora sì ch'io son contenta.".to_string()),
                translation: None,
                direction: None,
                group: None,
            },
        ];

        let translation = vec![
            Segment {
                id: "no-1-duettino-001".to_string(),
                segment_type: SegmentType::Sung,
                character: Some("FIGARO".to_string()),
                text: Some("Five... ten...".to_string()),
                translation: None,
                direction: None,
                group: None,
            },
            Segment {
                id: "no-1-duettino-002".to_string(),
                segment_type: SegmentType::Sung,
                character: Some("SUSANNA".to_string()),
                text: Some("How happy I am now.".to_string()),
                translation: None,
                direction: None,
                group: None,
            },
        ];

        align_segments(&mut original, &translation);

        assert_eq!(original[0].translation.as_deref(), Some("Five... ten..."));
        assert_eq!(original[1].translation.as_deref(), Some("How happy I am now."));
    }

    #[test]
    fn test_pipeline() {
        let elements = vec![
            ContentElement::ActHeader("Personaggi".to_string()),
            ContentElement::Text("Figaro - basso-baritono".to_string()),
            ContentElement::NumberLabel("Sinfonia".to_string()),
            ContentElement::ActHeader("ATTO PRIMO".to_string()),
            ContentElement::NumberLabel("N° 1: Duettino".to_string()),
            ContentElement::Character("FIGARO".to_string()),
            ContentElement::Text("Cinque... dieci...".to_string()),
            ContentElement::Character("SUSANNA".to_string()),
            ContentElement::Text("Ora sì ch'io son contenta.".to_string()),
        ];

        let result = pipeline(&elements);

        assert_eq!(result.cast.len(), 1);
        assert_eq!(result.cast[0].character, "Figaro");

        // overture (empty, retained) + duettino
        assert!(result.numbers.len() >= 1);

        // Find the duettino segments
        let duettino_segs: Vec<_> = result.segments.iter()
            .filter(|s| s.id.starts_with("no-1-duettino"))
            .collect();
        assert_eq!(duettino_segs.len(), 2);
        assert_eq!(duettino_segs[0].character.as_deref(), Some("FIGARO"));
        assert_eq!(duettino_segs[1].character.as_deref(), Some("SUSANNA"));
    }
}
