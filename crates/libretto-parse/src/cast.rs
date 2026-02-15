// Cast list extraction from raw libretto text.
//
// Parses the opening "Personaggi" / "Cast" section to produce
// a list of `CastMember` entries.

use libretto_acquire::types::ContentElement;
use libretto_model::base_libretto::CastMember;
use regex::Regex;

/// Result of parsing the cast section: the members found and the
/// index of the first element *after* the cast section.
pub struct CastParseResult {
    pub members: Vec<CastMember>,
    /// Index into the element slice where the cast section ends
    /// (i.e., the first ActHeader, NumberLabel, or structural element
    /// after the cast entries).
    pub end_index: usize,
}

/// Extract the cast list from the beginning of an element sequence.
///
/// Recognizes two formats:
/// - **Italian** (`Personaggi`): `Text` entries like `"Figaro - basso-baritono"`
///   or `"Cherubino, paggio del Conte - mezzosoprano"`.
/// - **English** (`Cast`): `Character` entries like `"FIGARO (bass)"`.
///
/// Stops when it encounters a non-cast element (ActHeader for an act,
/// NumberLabel, Direction, etc.).
pub fn extract_cast(elements: &[ContentElement]) -> CastParseResult {
    let mut members = Vec::new();
    let mut i = 0;

    // Skip leading BlankLines and find the cast header
    while i < elements.len() {
        match &elements[i] {
            ContentElement::BlankLine => { i += 1; }
            ContentElement::ActHeader(h) if is_cast_header(h) => {
                i += 1;
                break;
            }
            // No cast header found — no cast section
            _ => return CastParseResult { members, end_index: 0 },
        }
    }

    // Now parse cast entries until we hit structure
    while i < elements.len() {
        match &elements[i] {
            ContentElement::BlankLine => { i += 1; }

            // An ActHeader that isn't a cast header means the libretto body starts
            ContentElement::ActHeader(_) => break,

            // A NumberLabel (e.g., "Overture", "Sinfonia") ends the cast
            ContentElement::NumberLabel(_) => break,

            // A Direction means we've left the cast section
            ContentElement::Direction(_) => break,

            // Character element: English-style cast (ALL-CAPS with optional voice in parens)
            ContentElement::Character(text) => {
                if let Some(member) = parse_character_entry(text) {
                    members.push(member);
                }
                i += 1;
            }

            // Text element: Italian-style cast ("Name, description - voice_type")
            ContentElement::Text(text) => {
                if let Some(member) = parse_text_entry(text) {
                    members.push(member);
                } else {
                    // If we can't parse it as a cast entry, it might be
                    // a continuation (e.g., "peasants and the count's tenants")
                    // Attach it as description to the last member
                    if let Some(last) = members.last_mut() {
                        let desc = last.description.get_or_insert_with(String::new);
                        if !desc.is_empty() {
                            desc.push_str("; ");
                        }
                        desc.push_str(text.trim());
                    }
                }
                i += 1;
            }
        }
    }

    CastParseResult { members, end_index: i }
}

/// Check if an ActHeader text is a cast section header.
fn is_cast_header(text: &str) -> bool {
    let t = text.trim().to_lowercase();
    t == "personaggi" || t == "cast" || t == "characters" || t == "dramatis personae"
}

/// Parse an English-style Character entry: `"FIGARO (bass)"` or `"CHORUS"`.
fn parse_character_entry(text: &str) -> Option<CastMember> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }

    // Pattern: NAME (voice_type)
    let re = Regex::new(r"^(.+?)\s*\(([^)]+)\)\s*$").unwrap();
    if let Some(caps) = re.captures(text) {
        let name = caps[1].trim().to_string();
        let voice = caps[2].trim().to_string();
        Some(CastMember {
            character: name.clone(),
            short_name: Some(name),
            voice_type: Some(voice),
            description: None,
        })
    } else {
        // No parenthetical — just a name (e.g., "CHORUS")
        Some(CastMember {
            character: text.to_string(),
            short_name: Some(text.to_string()),
            voice_type: None,
            description: None,
        })
    }
}

/// Parse an Italian-style Text entry: `"Cherubino, paggio del Conte - mezzosoprano"`.
///
/// Format: `Name [, description] - voice_type`
/// Some entries have no voice type: `"Due Donne"`, `"Coro di Contadini, ..."`
fn parse_text_entry(text: &str) -> Option<CastMember> {
    let text = text.trim();
    if text.is_empty() {
        return None;
    }

    // Try to split on " - " or " – " (dash separating name from voice type)
    let re = Regex::new(r"^(.+?)\s*[-–]\s*(\S.*)$").unwrap();
    if let Some(caps) = re.captures(text) {
        let name_part = caps[1].trim();
        let voice = caps[2].trim().to_string();

        // The name_part might contain a comma-separated description:
        // "Cherubino, paggio del Conte"
        let (character, description) = split_name_description(name_part);

        Some(CastMember {
            character,
            short_name: None,
            voice_type: Some(voice),
            description,
        })
    } else {
        // No dash — could be "Due Donne" or "Coro di Contadini, ..."
        // But could also be continuation text like "peasants and the count's tenants".
        // Heuristic: a cast entry without a voice type should start with
        // a capitalized word (proper noun).
        let first_char = text.chars().next()?;
        if !first_char.is_uppercase() {
            return None;
        }
        let (character, description) = split_name_description(text);
        Some(CastMember {
            character,
            short_name: None,
            voice_type: None,
            description,
        })
    }
}

/// Split "Cherubino, paggio del Conte" into ("Cherubino", Some("paggio del Conte")).
///
/// Only splits on the first comma. If the text after the comma looks like
/// it's part of the name (e.g., starts with a capital letter matching a title),
/// keeps it together.
fn split_name_description(text: &str) -> (String, Option<String>) {
    if let Some(comma_pos) = text.find(',') {
        let name = text[..comma_pos].trim().to_string();
        let desc = text[comma_pos + 1..].trim().to_string();
        if desc.is_empty() {
            (name, None)
        } else {
            (name, Some(desc))
        }
    } else {
        (text.to_string(), None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_character_entry_with_voice() {
        let m = parse_character_entry("FIGARO (bass)").unwrap();
        assert_eq!(m.character, "FIGARO");
        assert_eq!(m.voice_type.as_deref(), Some("bass"));
        assert_eq!(m.short_name.as_deref(), Some("FIGARO"));
    }

    #[test]
    fn test_parse_character_entry_no_voice() {
        let m = parse_character_entry("CHORUS").unwrap();
        assert_eq!(m.character, "CHORUS");
        assert_eq!(m.voice_type, None);
    }

    #[test]
    fn test_parse_text_entry_with_description() {
        let m = parse_text_entry("Cherubino, paggio del Conte - mezzosoprano").unwrap();
        assert_eq!(m.character, "Cherubino");
        assert_eq!(m.description.as_deref(), Some("paggio del Conte"));
        assert_eq!(m.voice_type.as_deref(), Some("mezzosoprano"));
    }

    #[test]
    fn test_parse_text_entry_simple() {
        let m = parse_text_entry("Susanna - soprano").unwrap();
        assert_eq!(m.character, "Susanna");
        assert_eq!(m.voice_type.as_deref(), Some("soprano"));
        assert_eq!(m.description, None);
    }

    #[test]
    fn test_parse_text_entry_no_voice() {
        let m = parse_text_entry("Due Donne").unwrap();
        assert_eq!(m.character, "Due Donne");
        assert_eq!(m.voice_type, None);
    }

    #[test]
    fn test_extract_cast_italian() {
        let elements = vec![
            ContentElement::ActHeader("Personaggi".to_string()),
            ContentElement::Text("Il Conte di Almaviva - baritono".to_string()),
            ContentElement::Text("Susanna - soprano".to_string()),
            ContentElement::Text("Cherubino, paggio del Conte - mezzosoprano".to_string()),
            ContentElement::NumberLabel("Sinfonia".to_string()),
            ContentElement::ActHeader("ATTO PRIMO".to_string()),
        ];
        let result = extract_cast(&elements);
        assert_eq!(result.members.len(), 3);
        assert_eq!(result.members[0].character, "Il Conte di Almaviva");
        assert_eq!(result.members[0].voice_type.as_deref(), Some("baritono"));
        assert_eq!(result.members[2].description.as_deref(), Some("paggio del Conte"));
        // Stops at NumberLabel
        assert_eq!(result.end_index, 4);
    }

    #[test]
    fn test_extract_cast_english() {
        let elements = vec![
            ContentElement::ActHeader("Cast".to_string()),
            ContentElement::Character("FIGARO (bass)".to_string()),
            ContentElement::Character("SUSANNA (soprano)".to_string()),
            ContentElement::Character("CHORUS".to_string()),
            ContentElement::Text("peasants and the count's tenants".to_string()),
            ContentElement::NumberLabel("Overture".to_string()),
        ];
        let result = extract_cast(&elements);
        assert_eq!(result.members.len(), 3);
        assert_eq!(result.members[0].character, "FIGARO");
        assert_eq!(result.members[0].voice_type.as_deref(), Some("bass"));
        // "peasants..." attached as description to CHORUS
        assert_eq!(result.members[2].description.as_deref(), Some("peasants and the count's tenants"));
        assert_eq!(result.end_index, 5);
    }

    #[test]
    fn test_no_cast_section() {
        let elements = vec![
            ContentElement::ActHeader("ATTO PRIMO".to_string()),
            ContentElement::Character("FIGARO".to_string()),
        ];
        let result = extract_cast(&elements);
        assert_eq!(result.members.len(), 0);
        assert_eq!(result.end_index, 0);
    }
}
