// Structural splitting of raw libretto text.
//
// Splits raw text into acts, scenes, and musical numbers using
// markers like "ATTO PRIMO", "No. 1 - Duettino", "Recitativo", etc.

use libretto_acquire::types::ContentElement;
use libretto_model::base_libretto::NumberType;
use regex::Regex;

/// A raw musical number block: label + the elements belonging to it.
/// This is an intermediate representation before segment splitting.
#[derive(Debug, Clone)]
pub struct RawNumber {
    /// Display label (e.g., "N° 1: Duettino", "Sinfonia").
    pub label: String,
    /// Generated slug ID (e.g., "no-1-duettino", "overture", "rec-1a").
    pub id: String,
    /// Classified type of this number.
    pub number_type: NumberType,
    /// Act this number belongs to (e.g., "1", "2").
    pub act: String,
    /// Scene within the act, if known.
    pub scene: Option<String>,
    /// The content elements belonging to this number.
    pub elements: Vec<ContentElement>,
}

/// Split a flat element sequence into structured number blocks.
///
/// `elements` should start *after* the cast section (use `CastParseResult::end_index`).
///
/// Walk the elements tracking current act/scene. Each `NumberLabel` starts a new
/// block; each `ActHeader` updates the act counter. Text between an act header
/// and the first number label becomes an implicit recitative block.
pub fn split_into_numbers(elements: &[ContentElement]) -> Vec<RawNumber> {
    let mut numbers: Vec<RawNumber> = Vec::new();
    let mut current_act = String::new();
    let mut current_scene: Option<String> = None;
    let mut recit_counter: u32 = 0;

    for elem in elements {
        match elem {
            ContentElement::ActHeader(text) => {
                if let Some(act_num) = parse_act_number(text) {
                    current_act = act_num;
                    current_scene = None;
                }
                // Ignore non-act headers (like "Personaggi" leftovers)
            }

            ContentElement::NumberLabel(text) => {
                // Filter out noise entries that aren't real musical numbers
                if is_noise_label(text) {
                    continue;
                }

                let number_type = classify_number(text);
                let id = generate_id(text, &current_act, &number_type);

                numbers.push(RawNumber {
                    label: text.clone(),
                    id,
                    number_type,
                    act: current_act.clone(),
                    scene: current_scene.clone(),
                    elements: Vec::new(),
                });
            }

            // Content elements — attach to the current number block
            other => {
                // Skip blank lines before the first number
                if numbers.is_empty() {
                    if matches!(other, ContentElement::BlankLine) {
                        continue;
                    }
                    // Text before the first labeled number: create an implicit recitative
                    recit_counter += 1;
                    let id = if current_act.is_empty() {
                        format!("rec-{recit_counter}")
                    } else {
                        format!("rec-{}{}", current_act, char::from(b'a' - 1 + recit_counter as u8))
                    };
                    numbers.push(RawNumber {
                        label: "Recitativo".to_string(),
                        id,
                        number_type: NumberType::Recitative,
                        act: current_act.clone(),
                        scene: current_scene.clone(),
                        elements: Vec::new(),
                    });
                }
                numbers.last_mut().unwrap().elements.push(other.clone());
            }
        }
    }

    // Remove empty number blocks (can happen with consecutive structural markers)
    numbers.retain(|n| !n.elements.is_empty() || n.number_type == NumberType::Overture);

    numbers
}

/// Parse an act number from an ActHeader string.
///
/// Handles: "ATTO PRIMO", "ACT ONE", "ATTO SECONDO", "ACT 2", etc.
fn parse_act_number(text: &str) -> Option<String> {
    let t = text.trim().to_uppercase();

    // Italian ordinals
    if t.contains("PRIMO") || t.contains("FIRST") || t.contains("ONE") {
        return Some("1".to_string());
    }
    if t.contains("SECONDO") || t.contains("SECOND") || t.contains("TWO") {
        return Some("2".to_string());
    }
    if t.contains("TERZO") || t.contains("THIRD") || t.contains("THREE") {
        return Some("3".to_string());
    }
    if t.contains("QUARTO") || t.contains("FOURTH") || t.contains("FOUR") {
        return Some("4".to_string());
    }
    if t.contains("QUINTO") || t.contains("FIFTH") || t.contains("FIVE") {
        return Some("5".to_string());
    }

    // Numeric: "ACT 2", "ATTO 3"
    let re = Regex::new(r"(?i)(?:act|atto)\s+(\d+)").unwrap();
    if let Some(caps) = re.captures(&t) {
        return Some(caps[1].to_string());
    }

    None
}

/// Classify a NumberLabel into a NumberType.
fn classify_number(label: &str) -> NumberType {
    let lower = label.to_lowercase();

    if lower.contains("sinfonia") || lower.contains("overture") || lower.contains("ouverture") {
        return NumberType::Overture;
    }
    if lower.contains("finale") {
        return NumberType::Finale;
    }

    // Check for specific types (order matters: check compound types first)
    if lower.contains("recitativo") || lower.contains("recitative") {
        // "Recitativo ed Aria" — classify as the aria, not recitative
        if lower.contains("aria") {
            return NumberType::Aria;
        }
        return NumberType::Recitative;
    }
    if lower.contains("duettino") { return NumberType::Duettino; }
    if lower.contains("duetto") || lower.contains("duet") { return NumberType::Duet; }
    if lower.contains("terzetto") || lower.contains("trio") { return NumberType::Terzetto; }
    if lower.contains("quartetto") || lower.contains("quartet") { return NumberType::Quartet; }
    if lower.contains("quintetto") || lower.contains("quintet") { return NumberType::Quintet; }
    if lower.contains("sestetto") || lower.contains("sextet") { return NumberType::Sextet; }
    if lower.contains("cavatina") { return NumberType::Cavatina; }
    if lower.contains("canzone") { return NumberType::Canzone; }
    if lower.contains("coro") || lower.contains("chorus") { return NumberType::Chorus; }
    if lower.contains("aria") { return NumberType::Aria; }

    NumberType::Other
}

/// Generate a slug ID from a number label.
///
/// Examples:
/// - "N° 1: Duettino" → "no-1-duettino"
/// - "Sinfonia" → "overture"
/// - "N° 17: Recitativo ed Aria" → "no-17-recitativo-ed-aria"
fn generate_id(label: &str, act: &str, number_type: &NumberType) -> String {
    // Special case: overture
    if *number_type == NumberType::Overture {
        return "overture".to_string();
    }

    // Try to extract "N° 1: Duettino" → "no-1-duettino"
    let re = Regex::new(r"(?i)n[°o\.]\s*(\d+)\s*[:\-–]\s*(.+)").unwrap();
    if let Some(caps) = re.captures(label) {
        let num = &caps[1];
        let desc = caps[2].trim().to_lowercase();
        let desc_slug: String = desc
            .chars()
            .map(|c| if c.is_alphanumeric() || c == ' ' { c } else { ' ' })
            .collect::<String>()
            .split_whitespace()
            .collect::<Vec<_>>()
            .join("-");
        return format!("no-{num}-{desc_slug}");
    }

    // Try just a number: "N° 22"
    let re_num = Regex::new(r"(?i)n[°o\.]\s*(\d+)").unwrap();
    if let Some(caps) = re_num.captures(label) {
        return format!("no-{}", &caps[1]);
    }

    // Fallback: slugify the whole label
    let slug: String = label
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() || c == ' ' { c } else { ' ' })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("-");

    if slug.is_empty() {
        format!("number-act{act}")
    } else {
        slug
    }
}

/// Detect noise NumberLabel entries that aren't real musical numbers.
fn is_noise_label(text: &str) -> bool {
    let lower = text.to_lowercase();
    // "Symphony No.38 in D 'Prague'" — incidental catalog info
    if lower.starts_with("symphony") {
        return true;
    }
    // "Fin dell'opera" — end marker, not a number
    if lower.starts_with("fin ") || lower == "fine" {
        return true;
    }
    // "Lorenzo Da Ponte" — librettist name, not a number
    // Heuristic: no digits, no known type keywords
    let has_digit = text.chars().any(|c| c.is_ascii_digit());
    let has_keyword = ["aria", "duet", "terzet", "quartet", "quintet", "sextet",
        "cavatina", "canzone", "coro", "chorus", "finale", "recitativ",
        "overture", "sinfonia", "ouverture", "duettino"]
        .iter()
        .any(|kw| lower.contains(kw));
    if !has_digit && !has_keyword && !lower.starts_with("n°") && !lower.starts_with("no.") {
        return true;
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_act_number() {
        assert_eq!(parse_act_number("ATTO PRIMO"), Some("1".to_string()));
        assert_eq!(parse_act_number("ACT TWO"), Some("2".to_string()));
        assert_eq!(parse_act_number("ATTO TERZO"), Some("3".to_string()));
        assert_eq!(parse_act_number("ATTO QUARTO"), Some("4".to_string()));
        assert_eq!(parse_act_number("ACT 3"), Some("3".to_string()));
        assert_eq!(parse_act_number("Personaggi"), None);
    }

    #[test]
    fn test_classify_number() {
        assert_eq!(classify_number("N° 1: Duettino"), NumberType::Duettino);
        assert_eq!(classify_number("Sinfonia"), NumberType::Overture);
        assert_eq!(classify_number("N° 15: Finale"), NumberType::Finale);
        assert_eq!(classify_number("N° 17: Recitativo ed Aria"), NumberType::Aria);
        assert_eq!(classify_number("N° 8: Coro"), NumberType::Chorus);
        assert_eq!(classify_number("N° 18: Sestetto"), NumberType::Sextet);
    }

    #[test]
    fn test_generate_id() {
        assert_eq!(generate_id("Sinfonia", "1", &NumberType::Overture), "overture");
        assert_eq!(generate_id("N° 1: Duettino", "1", &NumberType::Duettino), "no-1-duettino");
        assert_eq!(generate_id("N° 17: Recitativo ed Aria", "3", &NumberType::Aria), "no-17-recitativo-ed-aria");
    }

    #[test]
    fn test_is_noise_label() {
        assert!(is_noise_label("Symphony No.38 in D 'Prague'"));
        assert!(is_noise_label("Fin dell'opera"));
        assert!(is_noise_label("Lorenzo Da Ponte"));
        assert!(!is_noise_label("N° 1: Duettino"));
        assert!(!is_noise_label("Sinfonia"));
        assert!(!is_noise_label("N° 22: Finale"));
    }

    #[test]
    fn test_split_into_numbers() {
        let elements = vec![
            ContentElement::ActHeader("ATTO PRIMO".to_string()),
            ContentElement::Direction("(A room in the castle)".to_string()),
            ContentElement::NumberLabel("N° 1: Duettino".to_string()),
            ContentElement::Character("FIGARO".to_string()),
            ContentElement::Text("Cinque... dieci...".to_string()),
            ContentElement::Character("SUSANNA".to_string()),
            ContentElement::Text("Ora sì ch'io son contenta.".to_string()),
            ContentElement::NumberLabel("N° 2: Duettino".to_string()),
            ContentElement::Character("FIGARO".to_string()),
            ContentElement::Text("Se a caso madama...".to_string()),
            ContentElement::ActHeader("ATTO SECONDO".to_string()),
            ContentElement::NumberLabel("N° 10: Cavatina".to_string()),
            ContentElement::Character("LA CONTESSA".to_string()),
            ContentElement::Text("Porgi, amor...".to_string()),
        ];

        let numbers = split_into_numbers(&elements);

        // Should have: implicit recitative (Direction before N°1), N°1, N°2, N°10
        assert_eq!(numbers.len(), 4);

        assert_eq!(numbers[0].number_type, NumberType::Recitative);
        assert_eq!(numbers[0].act, "1");

        assert_eq!(numbers[1].id, "no-1-duettino");
        assert_eq!(numbers[1].act, "1");
        assert_eq!(numbers[1].number_type, NumberType::Duettino);
        assert_eq!(numbers[1].elements.len(), 4); // FIGARO, text, SUSANNA, text

        assert_eq!(numbers[2].id, "no-2-duettino");
        assert_eq!(numbers[2].elements.len(), 2); // FIGARO, text

        assert_eq!(numbers[3].id, "no-10-cavatina");
        assert_eq!(numbers[3].act, "2");
        assert_eq!(numbers[3].elements.len(), 2);
    }

    #[test]
    fn test_noise_filtered() {
        let elements = vec![
            ContentElement::ActHeader("ATTO PRIMO".to_string()),
            ContentElement::NumberLabel("N° 9: Aria".to_string()),
            ContentElement::Character("FIGARO".to_string()),
            ContentElement::Text("Non più andrai...".to_string()),
            ContentElement::NumberLabel("Symphony No.38 in D 'Prague'".to_string()),
            ContentElement::ActHeader("ATTO SECONDO".to_string()),
        ];

        let numbers = split_into_numbers(&elements);
        assert_eq!(numbers.len(), 1);
        assert_eq!(numbers[0].id, "no-9-aria");
    }
}
