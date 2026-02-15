use serde::{Deserialize, Serialize};

/// A base libretto: the untimed, structured text of an opera.
///
/// This contains the full libretto organized by musical numbers, with
/// segment IDs that timing overlays reference. It is independent of
/// any particular recording.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BaseLibretto {
    pub version: String,
    pub opera: OperaMetadata,
    pub cast: Vec<CastMember>,
    pub numbers: Vec<MusicalNumber>,
}

/// Metadata about the opera itself.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OperaMetadata {
    pub title: String,
    pub composer: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub librettist: Option<String>,
    /// ISO 639-1 code for the original language (e.g., "it", "de", "fr").
    pub language: String,
    /// ISO 639-1 code for the translation language, if translations are included.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub translation_language: Option<String>,
    /// Year of the opera's premiere.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub year: Option<u16>,
}

/// A member of the cast list.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CastMember {
    /// Character name as it appears in the libretto (e.g., "Il Conte d'Almaviva").
    pub character: String,
    /// Normalized short name used in segment attributions (e.g., "IL CONTE").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub short_name: Option<String>,
    /// Voice type (e.g., "baritone", "soprano").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub voice_type: Option<String>,
    /// Description or role info (e.g., "page to the Count").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// A musical number within the opera (aria, duet, recitative, finale, etc.).
///
/// Each number corresponds roughly to one track in most recordings,
/// though some tracks may contain multiple numbers.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MusicalNumber {
    /// Unique identifier for this number (e.g., "no-1-duettino", "rec-1a", "overture").
    pub id: String,
    /// Display label (e.g., "No. 1 - Duettino", "Recitativo").
    pub label: String,
    /// The type of musical number.
    pub number_type: NumberType,
    /// Act this number belongs to (e.g., "1", "2").
    pub act: String,
    /// Scene within the act, if applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scene: Option<String>,
    /// Ordered segments of text within this number.
    pub segments: Vec<Segment>,
}

/// Classification of a musical number.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NumberType {
    Overture,
    Aria,
    Duet,
    Duettino,
    Terzetto,
    Quartet,
    Quintet,
    Sextet,
    Cavatina,
    Canzone,
    Chorus,
    Finale,
    Recitative,
    /// Catch-all for types not in the enum.
    Other,
}

/// A segment of libretto text within a musical number.
///
/// This is the fundamental unit that timing overlays reference by `id`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Segment {
    /// Unique identifier within the base libretto (e.g., "no-1-001").
    pub id: String,
    /// The type of content in this segment.
    #[serde(default = "default_segment_type")]
    pub segment_type: SegmentType,
    /// Character name(s) singing/speaking (e.g., "FIGARO", "SUSANNA, FIGARO").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub character: Option<String>,
    /// Original language text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Translation text.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub translation: Option<String>,
    /// Stage direction associated with this segment.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    /// Ensemble group tag. Segments with the same group within a number are
    /// sung simultaneously and should be displayed together.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

/// Type of content in a segment.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SegmentType {
    Sung,
    Spoken,
    Direction,
    Interlude,
}

fn default_segment_type() -> SegmentType {
    SegmentType::Sung
}

impl BaseLibretto {
    /// Create a new base libretto with the given metadata.
    pub fn new(opera: OperaMetadata) -> Self {
        Self {
            version: "1.0".to_string(),
            opera,
            cast: Vec::new(),
            numbers: Vec::new(),
        }
    }

    /// Get all segment IDs in the libretto, in order.
    pub fn segment_ids(&self) -> Vec<&str> {
        self.numbers
            .iter()
            .flat_map(|n| n.segments.iter().map(|s| s.id.as_str()))
            .collect()
    }

    /// Look up a segment by ID.
    pub fn find_segment(&self, id: &str) -> Option<&Segment> {
        self.numbers
            .iter()
            .flat_map(|n| n.segments.iter())
            .find(|s| s.id == id)
    }

    /// Look up a musical number by ID.
    pub fn find_number(&self, id: &str) -> Option<&MusicalNumber> {
        self.numbers.iter().find(|n| n.id == id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_libretto() -> BaseLibretto {
        let mut libretto = BaseLibretto::new(OperaMetadata {
            title: "Le nozze di Figaro".to_string(),
            composer: "Wolfgang Amadeus Mozart".to_string(),
            librettist: Some("Lorenzo Da Ponte".to_string()),
            language: "it".to_string(),
            translation_language: Some("en".to_string()),
            year: Some(1786),
        });

        libretto.cast.push(CastMember {
            character: "Figaro".to_string(),
            short_name: Some("FIGARO".to_string()),
            voice_type: Some("bass-baritone".to_string()),
            description: None,
        });

        libretto.numbers.push(MusicalNumber {
            id: "no-1-duettino".to_string(),
            label: "No. 1 - Duettino".to_string(),
            number_type: NumberType::Duettino,
            act: "1".to_string(),
            scene: Some("1".to_string()),
            segments: vec![
                Segment {
                    id: "no-1-001".to_string(),
                    segment_type: SegmentType::Sung,
                    character: Some("FIGARO".to_string()),
                    text: Some("Cinque... dieci... venti...".to_string()),
                    translation: Some("Five... ten... twenty...".to_string()),
                    direction: None,
                    group: None,
                },
                Segment {
                    id: "no-1-002".to_string(),
                    segment_type: SegmentType::Sung,
                    character: Some("SUSANNA".to_string()),
                    text: Some("Ora sì ch'io son contenta.".to_string()),
                    translation: Some("How happy I am now.".to_string()),
                    direction: None,
                    group: None,
                },
            ],
        });

        libretto
    }

    #[test]
    fn test_segment_ids() {
        let libretto = sample_libretto();
        let ids = libretto.segment_ids();
        assert_eq!(ids, vec!["no-1-001", "no-1-002"]);
    }

    #[test]
    fn test_find_segment() {
        let libretto = sample_libretto();
        let seg = libretto.find_segment("no-1-001").unwrap();
        assert_eq!(seg.character.as_deref(), Some("FIGARO"));
        assert!(libretto.find_segment("nonexistent").is_none());
    }

    #[test]
    fn test_find_number() {
        let libretto = sample_libretto();
        let num = libretto.find_number("no-1-duettino").unwrap();
        assert_eq!(num.segments.len(), 2);
    }

    #[test]
    fn test_json_roundtrip() {
        let libretto = sample_libretto();
        let json = serde_json::to_string_pretty(&libretto).unwrap();
        let parsed: BaseLibretto = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.opera.title, "Le nozze di Figaro");
        assert_eq!(parsed.numbers.len(), 1);
        assert_eq!(parsed.numbers[0].segments.len(), 2);
    }
}
