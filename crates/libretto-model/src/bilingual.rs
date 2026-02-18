// Bilingual libretto transformation.
//
// Reads an acquired bilingual JSON (from murashev or similar source) and
// restructures it into front_matter (source, languages, cast) plus a
// numbers array with segments containing text and translations.

use serde::{Deserialize, Serialize};
use serde_json::Value;

// ---------------------------------------------------------------------------
// Input types (matching the AcquiredLibretto shape from libretto-acquire)
// ---------------------------------------------------------------------------

/// Raw acquired bilingual libretto as produced by the acquire step.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawBilingual {
    pub source: Value,
    pub text_lang: String,
    pub translation_lang: String,
    pub rows: Vec<RawRow>,
}

/// A single row from the bilingual table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawRow {
    pub index: usize,
    #[serde(default)]
    pub text_elements: Vec<RawElement>,
    #[serde(default)]
    pub translation_elements: Vec<RawElement>,
}

/// A content element with type tag and text.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawElement {
    #[serde(rename = "type")]
    pub element_type: String,
    #[serde(default)]
    pub text: String,
}

// ---------------------------------------------------------------------------
// Output types
// ---------------------------------------------------------------------------

/// Restructured bilingual libretto with front matter and structured numbers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Libretto {
    pub front_matter: LibrettoFrontMatter,
    pub numbers: Vec<LibrettoNumber>,
}

/// Front matter section.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibrettoFrontMatter {
    pub source: Value,
    pub languages: LibrettoLanguages,
    pub cast: LibrettoCast,
}

/// Language codes: original text language and translation language.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibrettoLanguages {
    pub text: String,
    pub translation: String,
}

/// Cast data extracted from the pre-NumberLabel portion of the rows.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibrettoCast {
    pub entries: Vec<CastEntry>,
}

/// A single cast entry with text (original language) and translation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CastEntry {
    pub text: String,
    pub translation: String,
}

/// A musical number within the libretto.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibrettoNumber {
    pub id: u32,
    pub title: String,
    pub form: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub act: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub characters: Vec<String>,
    pub segments: Vec<LibrettoSegment>,
}

/// A segment: one character's block of text with translation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LibrettoSegment {
    pub id: String,
    pub character: String,
    pub text: String,
    pub translation: String,
}

// ---------------------------------------------------------------------------
// Internal parsing types
// ---------------------------------------------------------------------------

struct ParsedNumber {
    title: String,
    act: Option<String>,
    directions: Vec<String>,
    segments: Vec<ParsedSegment>,
}

struct ParsedSegment {
    character: String,
    lines: Vec<String>,
}

// ---------------------------------------------------------------------------
// Transformation
// ---------------------------------------------------------------------------

/// Transform a raw bilingual into structured libretto with numbers and segments.
pub fn build_libretto(raw: &RawBilingual) -> Libretto {
    let (cast_text, cast_trans, content_text, content_trans) = extract_cast_and_content(raw);
    let trans_numbers = parse_stream(&content_trans);
    let text_numbers = parse_stream(&content_text);
    let numbers = merge_numbers(trans_numbers, text_numbers);

    Libretto {
        front_matter: LibrettoFrontMatter {
            source: raw.source.clone(),
            languages: LibrettoLanguages {
                text: raw.text_lang.clone(),
                translation: raw.translation_lang.clone(),
            },
            cast: LibrettoCast {
                entries: pair_cast_entries(&cast_text, &cast_trans),
            },
        },
        numbers,
    }
}

/// Extract cast elements (before first NumberLabel) and flatten content elements.
///
/// Returns (cast_text, cast_trans, content_text, content_trans).
fn extract_cast_and_content(
    raw: &RawBilingual,
) -> (Vec<RawElement>, Vec<RawElement>, Vec<RawElement>, Vec<RawElement>) {
    let mut cast_text = Vec::new();
    let mut cast_trans = Vec::new();
    let mut content_text = Vec::new();
    let mut content_trans = Vec::new();
    let mut found_nl = false;

    for row in &raw.rows {
        if found_nl {
            content_trans.extend(row.translation_elements.iter().cloned());
            // Filter noise NumberLabels from text: if translation has no NL in
            // this row, any text NL is noise (e.g. "Symphony" ads). Truncate at
            // the noise NL to avoid contaminating segment text.
            let trans_has_nl = row.translation_elements.iter()
                .any(|e| e.element_type == "NumberLabel");
            if trans_has_nl {
                content_text.extend(row.text_elements.iter().cloned());
            } else {
                let noise_pos = row.text_elements.iter()
                    .position(|e| e.element_type == "NumberLabel");
                match noise_pos {
                    Some(pos) => content_text.extend(row.text_elements[..pos].iter().cloned()),
                    None => content_text.extend(row.text_elements.iter().cloned()),
                }
            }
            continue;
        }
        let nl_trans = row.translation_elements.iter().position(|e| e.element_type == "NumberLabel");
        let nl_text = row.text_elements.iter().position(|e| e.element_type == "NumberLabel");

        if nl_trans.is_none() && nl_text.is_none() {
            cast_text.extend(row.text_elements.iter().filter(|e| !e.text.is_empty()).cloned());
            cast_trans.extend(row.translation_elements.iter().filter(|e| !e.text.is_empty()).cloned());
        } else {
            found_nl = true;
            let sp_trans = nl_trans.unwrap_or(row.translation_elements.len());
            let sp_text = nl_text.unwrap_or(row.text_elements.len());
            cast_trans.extend(row.translation_elements[..sp_trans].iter().filter(|e| !e.text.is_empty()).cloned());
            cast_text.extend(row.text_elements[..sp_text].iter().filter(|e| !e.text.is_empty()).cloned());
            content_trans.extend(row.translation_elements[sp_trans..].iter().cloned());
            content_text.extend(row.text_elements[sp_text..].iter().cloned());
        }
    }
    (cast_text, cast_trans, content_text, content_trans)
}

/// Parse a flat element stream into numbers with segments.
fn parse_stream(elements: &[RawElement]) -> Vec<ParsedNumber> {
    let mut numbers: Vec<ParsedNumber> = Vec::new();
    let mut current_act: Option<String> = None;
    let mut pending_dirs: Vec<String> = Vec::new();
    let mut cur_num: Option<ParsedNumber> = None;
    let mut cur_seg: Option<ParsedSegment> = None;

    for elem in elements {
        match elem.element_type.as_str() {
            "ActHeader" => {
                if let Some(act) = extract_act_from_header(&elem.text) {
                    current_act = Some(act);
                }
            }
            "Direction" => {
                if cur_seg.is_none() {
                    pending_dirs.push(elem.text.clone());
                }
            }
            "NumberLabel" => {
                if let Some(seg) = cur_seg.take() {
                    if let Some(ref mut n) = cur_num { n.segments.push(seg); }
                }
                if let Some(n) = cur_num.take() { numbers.push(n); }
                cur_num = Some(ParsedNumber {
                    title: elem.text.clone(),
                    act: current_act.clone(),
                    directions: std::mem::take(&mut pending_dirs),
                    segments: Vec::new(),
                });
            }
            "Character" => {
                if let Some(seg) = cur_seg.take() {
                    if let Some(ref mut n) = cur_num { n.segments.push(seg); }
                }
                if !pending_dirs.is_empty() {
                    if let Some(ref mut n) = cur_num {
                        n.directions.extend(pending_dirs.drain(..));
                    }
                }
                cur_seg = Some(ParsedSegment {
                    character: elem.text.clone(),
                    lines: Vec::new(),
                });
            }
            "Text" => {
                if let Some(ref mut seg) = cur_seg {
                    if !elem.text.is_empty() { seg.lines.push(elem.text.clone()); }
                }
            }
            _ => {}
        }
    }
    if let Some(seg) = cur_seg.take() {
        if let Some(ref mut n) = cur_num { n.segments.push(seg); }
    }
    if let Some(n) = cur_num.take() { numbers.push(n); }
    numbers
}

/// Merge translation numbers (lang1) with text numbers (lang2).
fn merge_numbers(trans: Vec<ParsedNumber>, text: Vec<ParsedNumber>) -> Vec<LibrettoNumber> {
    let mut result = Vec::new();
    let mut ti = 0;

    for (i, tn) in trans.iter().enumerate() {
        let id = (i + 1) as u32;
        let slug = slugify(&tn.title);
        let direction = if tn.directions.is_empty() {
            None
        } else {
            Some(tn.directions.join("\n"))
        };

        let text_num = find_matching(&text, &mut ti, tn);

        let segments: Vec<LibrettoSegment> = tn.segments.iter().enumerate().map(|(si, tseg)| {
            let txt = text_num
                .and_then(|n| n.segments.get(si))
                .map(|s| s.lines.join("\n"))
                .unwrap_or_default();
            LibrettoSegment {
                id: format!("{}-{}", slug, si + 1),
                character: tseg.character.clone(),
                text: txt,
                translation: tseg.lines.join("\n"),
            }
        }).collect();

        // Collect unique characters in segment order.
        // Split comma-separated names (e.g. "SUSANNA, FIGARO") and trim
        // trailing punctuation so "SUSANNA." becomes "SUSANNA".
        let mut characters: Vec<String> = Vec::new();
        for seg in &segments {
            for name in seg.character.split(',') {
                let clean = name.trim().trim_end_matches(|c: char| c.is_ascii_punctuation());
                if !clean.is_empty() && !characters.iter().any(|c| c == clean) {
                    characters.push(clean.to_string());
                }
            }
        }

        result.push(LibrettoNumber {
            id,
            title: tn.title.clone(),
            form: extract_content_type(&tn.title),
            act: tn.act.clone(),
            direction,
            characters,
            segments,
        });
    }
    result
}

/// Match numbers by structural shape, not character names (which are translated).
/// Both streams share the same row order; only noise NumberLabels in lang2 differ.
fn find_matching<'a>(
    text_numbers: &'a [ParsedNumber], ti: &mut usize, trans: &ParsedNumber,
) -> Option<&'a ParsedNumber> {
    while *ti < text_numbers.len() {
        let c = &text_numbers[*ti];
        // Both have segments → match (same structural position)
        if !trans.segments.is_empty() && !c.segments.is_empty() {
            *ti += 1;
            return Some(c);
        }
        // Both have no segments → match (e.g., Overture / End of Opera)
        if trans.segments.is_empty() && c.segments.is_empty() {
            *ti += 1;
            return Some(c);
        }
        // Mismatch: candidate has no segments but trans does → noise, skip
        *ti += 1;
    }
    None
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Pair cast elements from both languages into CastEntry items.
fn pair_cast_entries(text_elems: &[RawElement], trans_elems: &[RawElement]) -> Vec<CastEntry> {
    let len = text_elems.len().max(trans_elems.len());
    (0..len)
        .map(|i| CastEntry {
            text: text_elems.get(i).map(|e| e.text.clone()).unwrap_or_default(),
            translation: trans_elems.get(i).map(|e| e.text.clone()).unwrap_or_default(),
        })
        .collect()
}

fn extract_content_type(title: &str) -> String {
    if let Some(pos) = title.rfind(':') {
        title[pos + 1..].trim().to_lowercase()
    } else {
        title.trim().to_lowercase()
    }
}

fn slugify(title: &str) -> String {
    let mut result = String::new();
    let mut prev_dash = false;
    for c in title.to_lowercase().chars() {
        if c.is_alphanumeric() {
            result.push(c);
            prev_dash = false;
        } else if !prev_dash && !result.is_empty() {
            result.push('-');
            prev_dash = true;
        }
    }
    result.trim_end_matches('-').to_string()
}

fn extract_act_from_header(text: &str) -> Option<String> {
    let t = text.to_uppercase();
    if t.contains("ONE") || t.contains("PRIMO") || t == "ACT 1" { return Some("1".into()); }
    if t.contains("TWO") || t.contains("SECONDO") || t == "ACT 2" { return Some("2".into()); }
    if t.contains("THREE") || t.contains("TERZO") || t == "ACT 3" { return Some("3".into()); }
    if t.contains("FOUR") || t.contains("QUARTO") || t == "ACT 4" { return Some("4".into()); }
    if t.contains("FIVE") || t.contains("QUINTO") || t == "ACT 5" { return Some("5".into()); }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn elem(etype: &str, text: &str) -> RawElement {
        RawElement { element_type: etype.to_string(), text: text.to_string() }
    }

    #[test]
    fn test_parse_stream() {
        let elements = vec![
            elem("NumberLabel", "Overture"),
            elem("ActHeader", "ACT ONE"),
            elem("Direction", "A castle near Seville"),
            elem("NumberLabel", "No. 1: Duettino"),
            elem("Character", "FIGARO"),
            elem("Text", "Five...ten..."),
            elem("Text", "twenty...thirty..."),
            elem("Character", "SUSANNA"),
            elem("Text", "How happy I am now;"),
            elem("Direction", "etc."),
            elem("NumberLabel", "No. 2: Duettino"),
            elem("Character", "FIGARO"),
            elem("Text", "If you want to dance,"),
        ];
        let numbers = parse_stream(&elements);
        assert_eq!(numbers.len(), 3);
        assert_eq!(numbers[0].title, "Overture");
        assert_eq!(numbers[0].act, None);
        assert!(numbers[0].segments.is_empty());
        assert_eq!(numbers[1].title, "No. 1: Duettino");
        assert_eq!(numbers[1].act.as_deref(), Some("1"));
        assert_eq!(numbers[1].directions, vec!["A castle near Seville"]);
        assert_eq!(numbers[1].segments.len(), 2);
        assert_eq!(numbers[1].segments[0].character, "FIGARO");
        assert_eq!(numbers[1].segments[0].lines, vec!["Five...ten...", "twenty...thirty..."]);
        assert_eq!(numbers[1].segments[1].character, "SUSANNA");
        assert_eq!(numbers[1].segments[1].lines, vec!["How happy I am now;"]);
        assert_eq!(numbers[2].segments.len(), 1);
    }

    #[test]
    fn test_build_libretto_full() {
        let raw = RawBilingual {
            source: serde_json::json!({"url": "test"}),
            text_lang: "it".to_string(),
            translation_lang: "en".to_string(),
            rows: vec![
                RawRow { index: 0, text_elements: vec![], translation_elements: vec![] },
                RawRow {
                    index: 1,
                    translation_elements: vec![
                        elem("Character", "FIGARO (bass)"),
                        elem("NumberLabel", "Overture"),
                    ],
                    text_elements: vec![
                        elem("Text", "Figaro - basso"),
                        elem("NumberLabel", "Sinfonia"),
                    ],
                },
                RawRow {
                    index: 2,
                    translation_elements: vec![
                        elem("ActHeader", "ACT ONE"),
                        elem("Direction", "A castle"),
                        elem("NumberLabel", "No. 1: Duettino"),
                        elem("Character", "FIGARO"),
                        elem("Text", "Five...ten..."),
                        elem("Character", "SUSANNA"),
                        elem("Text", "How happy!"),
                    ],
                    text_elements: vec![
                        elem("ActHeader", "ATTO PRIMO"),
                        elem("Direction", "Un castello"),
                        elem("NumberLabel", "N\u{00b0} 1: Duettino"),
                        elem("Character", "FIGARO"),
                        elem("Text", "Cinque...dieci..."),
                        elem("Character", "SUSANNA"),
                        elem("Text", "Ora si!"),
                    ],
                },
            ],
        };
        let lib = build_libretto(&raw);
        assert_eq!(lib.front_matter.languages.text, "it");
        assert_eq!(lib.front_matter.languages.translation, "en");
        assert_eq!(lib.front_matter.cast.entries.len(), 1);
        assert_eq!(lib.front_matter.cast.entries[0].text, "Figaro - basso");
        assert_eq!(lib.front_matter.cast.entries[0].translation, "FIGARO (bass)");
        assert_eq!(lib.numbers.len(), 2);
        assert_eq!(lib.numbers[0].title, "Overture");
        assert_eq!(lib.numbers[0].form, "overture");
        assert!(lib.numbers[0].characters.is_empty());
        assert!(lib.numbers[0].segments.is_empty());
        assert_eq!(lib.numbers[1].title, "No. 1: Duettino");
        assert_eq!(lib.numbers[1].form, "duettino");
        assert_eq!(lib.numbers[1].characters, vec!["FIGARO", "SUSANNA"]);
        assert_eq!(lib.numbers[1].act.as_deref(), Some("1"));
        assert_eq!(lib.numbers[1].direction.as_deref(), Some("A castle"));
        assert_eq!(lib.numbers[1].segments.len(), 2);
        assert_eq!(lib.numbers[1].segments[0].id, "no-1-duettino-1");
        assert_eq!(lib.numbers[1].segments[0].character, "FIGARO");
        assert_eq!(lib.numbers[1].segments[0].text, "Cinque...dieci...");
        assert_eq!(lib.numbers[1].segments[0].translation, "Five...ten...");
        assert_eq!(lib.numbers[1].segments[1].text, "Ora si!");
        assert_eq!(lib.numbers[1].segments[1].translation, "How happy!");
    }

    #[test]
    fn test_extract_content_type() {
        assert_eq!(extract_content_type("Overture"), "overture");
        assert_eq!(extract_content_type("No. 1: Duettino"), "duettino");
        assert_eq!(extract_content_type("No. 17: Recitative and Aria"), "recitative and aria");
        assert_eq!(extract_content_type("End of the Opera"), "end of the opera");
    }

    #[test]
    fn test_slugify() {
        assert_eq!(slugify("Overture"), "overture");
        assert_eq!(slugify("No. 1: Duettino"), "no-1-duettino");
        assert_eq!(slugify("No. 17: Recitative and Aria"), "no-17-recitative-and-aria");
    }

    #[test]
    fn test_noise_skipped_in_merge() {
        let trans = vec![
            ParsedNumber { title: "Overture".into(), act: None, directions: vec![], segments: vec![] },
            ParsedNumber {
                title: "No. 1: Aria".into(), act: Some("1".into()), directions: vec![],
                segments: vec![ParsedSegment { character: "FIGARO".into(), lines: vec!["Five".into()] }],
            },
        ];
        let text = vec![
            ParsedNumber { title: "Sinfonia".into(), act: None, directions: vec![], segments: vec![] },
            ParsedNumber { title: "Symphony No.38".into(), act: None, directions: vec![], segments: vec![] },
            ParsedNumber {
                title: "N 1: Aria".into(), act: Some("1".into()), directions: vec![],
                segments: vec![ParsedSegment { character: "FIGARO".into(), lines: vec!["Cinque".into()] }],
            },
        ];
        let numbers = merge_numbers(trans, text);
        assert_eq!(numbers.len(), 2);
        assert_eq!(numbers[0].title, "Overture");
        assert_eq!(numbers[1].segments[0].text, "Cinque");
        assert_eq!(numbers[1].segments[0].translation, "Five");
    }
}
