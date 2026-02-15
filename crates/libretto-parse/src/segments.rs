// Segment splitting within musical numbers.
//
// Within each number, splits text by character (ALL-CAPS detection),
// separates stage directions from sung text, and generates segment IDs.

use libretto_acquire::types::ContentElement;
use libretto_model::base_libretto::{Segment, SegmentType};

use crate::structure::RawNumber;

/// Split a RawNumber's elements into ordered Segments.
///
/// Each `Character` element starts a new segment attributed to that character.
/// `Text` elements accumulate into the current segment's `text` field.
/// `Direction` elements become either:
/// - A standalone direction segment (if no character context), or
/// - Attached to the current segment's `direction` field.
/// `BlankLine` elements are ignored (they were stanza separators).
pub fn split_segments(number: &RawNumber) -> Vec<Segment> {
    let mut segments: Vec<Segment> = Vec::new();
    let mut seq: u32 = 0;
    let mut current_character: Option<String> = None;

    for elem in &number.elements {
        match elem {
            ContentElement::Character(name) => {
                current_character = Some(name.clone());
                // Start a new segment for this character
                seq += 1;
                segments.push(Segment {
                    id: format!("{}-{:03}", number.id, seq),
                    segment_type: SegmentType::Sung,
                    character: Some(name.clone()),
                    text: None,
                    translation: None,
                    direction: None,
                    group: None,
                });
            }

            ContentElement::Text(text) => {
                let text = text.trim();
                if text.is_empty() {
                    continue;
                }

                if let Some(seg) = segments.last_mut() {
                    // Append to current segment's text
                    if let Some(existing) = &mut seg.text {
                        existing.push('\n');
                        existing.push_str(text);
                    } else {
                        seg.text = Some(text.to_string());
                    }
                } else {
                    // Text before any character — create an unattributed segment
                    seq += 1;
                    segments.push(Segment {
                        id: format!("{}-{:03}", number.id, seq),
                        segment_type: SegmentType::Sung,
                        character: current_character.clone(),
                        text: Some(text.to_string()),
                        translation: None,
                        direction: None,
                        group: None,
                    });
                }
            }

            ContentElement::Direction(text) => {
                let text = text.trim();
                if text.is_empty() {
                    continue;
                }

                if let Some(seg) = segments.last_mut() {
                    // Attach direction to the current segment
                    if let Some(existing) = &mut seg.direction {
                        existing.push(' ');
                        existing.push_str(text);
                    } else {
                        seg.direction = Some(text.to_string());
                    }
                } else {
                    // Standalone direction before any character
                    seq += 1;
                    segments.push(Segment {
                        id: format!("{}-{:03}", number.id, seq),
                        segment_type: SegmentType::Direction,
                        character: None,
                        text: None,
                        translation: None,
                        direction: Some(text.to_string()),
                        group: None,
                    });
                }
            }

            ContentElement::BlankLine => {
                // Ignored — stanza separators don't create segments
            }

            // ActHeader and NumberLabel shouldn't appear inside a number's elements
            _ => {}
        }
    }

    segments
}

#[cfg(test)]
mod tests {
    use super::*;
    use libretto_model::base_libretto::NumberType;

    fn make_number(id: &str, elements: Vec<ContentElement>) -> RawNumber {
        RawNumber {
            label: id.to_string(),
            id: id.to_string(),
            number_type: NumberType::Duettino,
            act: "1".to_string(),
            scene: None,
            elements,
        }
    }

    #[test]
    fn test_basic_segments() {
        let number = make_number("no-1-duettino", vec![
            ContentElement::Character("FIGARO".to_string()),
            ContentElement::Text("Cinque... dieci...".to_string()),
            ContentElement::Character("SUSANNA".to_string()),
            ContentElement::Text("Ora sì ch'io son contenta.".to_string()),
        ]);

        let segs = split_segments(&number);
        assert_eq!(segs.len(), 2);

        assert_eq!(segs[0].id, "no-1-duettino-001");
        assert_eq!(segs[0].character.as_deref(), Some("FIGARO"));
        assert_eq!(segs[0].text.as_deref(), Some("Cinque... dieci..."));

        assert_eq!(segs[1].id, "no-1-duettino-002");
        assert_eq!(segs[1].character.as_deref(), Some("SUSANNA"));
        assert_eq!(segs[1].text.as_deref(), Some("Ora sì ch'io son contenta."));
    }

    #[test]
    fn test_multiline_text() {
        let number = make_number("no-1-duettino", vec![
            ContentElement::Character("FIGARO".to_string()),
            ContentElement::Text("Cinque... dieci...".to_string()),
            ContentElement::Text("venti... trenta...".to_string()),
        ]);

        let segs = split_segments(&number);
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].text.as_deref(), Some("Cinque... dieci...\nventi... trenta..."));
    }

    #[test]
    fn test_direction_attached() {
        let number = make_number("no-1-duettino", vec![
            ContentElement::Character("FIGARO".to_string()),
            ContentElement::Text("Cinque...".to_string()),
            ContentElement::Direction("(measuring the room)".to_string()),
        ]);

        let segs = split_segments(&number);
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].text.as_deref(), Some("Cinque..."));
        assert_eq!(segs[0].direction.as_deref(), Some("(measuring the room)"));
    }

    #[test]
    fn test_standalone_direction() {
        let number = make_number("rec-1a", vec![
            ContentElement::Direction("(A half-furnished room)".to_string()),
            ContentElement::Character("FIGARO".to_string()),
            ContentElement::Text("Cinque...".to_string()),
        ]);

        let segs = split_segments(&number);
        assert_eq!(segs.len(), 2);
        assert_eq!(segs[0].segment_type, SegmentType::Direction);
        assert_eq!(segs[0].direction.as_deref(), Some("(A half-furnished room)"));
        assert_eq!(segs[0].character, None);
        assert_eq!(segs[1].character.as_deref(), Some("FIGARO"));
    }

    #[test]
    fn test_blank_lines_ignored() {
        let number = make_number("no-1-duettino", vec![
            ContentElement::Character("FIGARO".to_string()),
            ContentElement::Text("Cinque...".to_string()),
            ContentElement::BlankLine,
            ContentElement::Text("dieci...".to_string()),
        ]);

        let segs = split_segments(&number);
        assert_eq!(segs.len(), 1);
        assert_eq!(segs[0].text.as_deref(), Some("Cinque...\ndieci..."));
    }
}
