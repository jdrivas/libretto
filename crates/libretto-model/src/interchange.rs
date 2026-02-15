use serde::{Deserialize, Serialize};

/// The full interchange format: a timed libretto for a complete opera recording.
///
/// This is the format consumed by display systems (e.g., roon-rd).
/// It combines opera metadata, track metadata, and timed text segments
/// into a single self-contained document.
///
/// See INTERCHANGE_FORMAT.md for the full specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterchangeLibretto {
    pub version: String,
    pub opera: InterchangeOpera,
    pub tracks: Vec<InterchangeTrack>,
}

/// Opera metadata in the interchange format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterchangeOpera {
    pub title: String,
    pub composer: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub librettist: Option<String>,
    pub language: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub translation_language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub year: Option<u16>,
}

/// A track in the interchange format, containing timed segments.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterchangeTrack {
    pub track_id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artist: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disc_number: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track_number: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub act: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scene: Option<String>,
    pub segments: Vec<InterchangeSegment>,
}

/// A timed text segment in the interchange format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InterchangeSegment {
    pub start: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub end: Option<f64>,
    #[serde(default = "default_type", skip_serializing_if = "is_default_type")]
    #[serde(rename = "type")]
    pub segment_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub character: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub translation: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub direction: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub act: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scene: Option<String>,
    /// Ensemble group tag. Segments with the same group within a track are
    /// sung simultaneously and should be displayed together.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<String>,
}

fn default_type() -> String {
    "sung".to_string()
}

fn is_default_type(s: &str) -> bool {
    s == "sung"
}

impl InterchangeTrack {
    /// Find the active segment at the given playback time (seconds).
    ///
    /// Returns the last segment whose `start` is <= the given time.
    pub fn segment_at(&self, time: f64) -> Option<&InterchangeSegment> {
        self.segments
            .iter()
            .rev()
            .find(|s| s.start <= time)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_segment_at() {
        let track = InterchangeTrack {
            track_id: "act-1".to_string(),
            title: "Act I".to_string(),
            album: None,
            artist: None,
            disc_number: None,
            track_number: None,
            duration_seconds: Some(100.0),
            act: None,
            scene: None,
            segments: vec![
                InterchangeSegment {
                    start: 0.0,
                    end: Some(10.0),
                    segment_type: "interlude".to_string(),
                    character: None,
                    text: None,
                    translation: None,
                    direction: Some("Overture begins.".to_string()),
                    act: None,
                    scene: None,
                    group: None,
                },
                InterchangeSegment {
                    start: 10.0,
                    end: Some(25.0),
                    segment_type: "sung".to_string(),
                    character: Some("FIGARO".to_string()),
                    text: Some("Cinque... dieci...".to_string()),
                    translation: Some("Five... ten...".to_string()),
                    direction: None,
                    act: None,
                    scene: None,
                    group: None,
                },
            ],
        };

        assert!(track.segment_at(-1.0).is_none());

        let seg = track.segment_at(5.0).unwrap();
        assert_eq!(seg.direction.as_deref(), Some("Overture begins."));

        let seg = track.segment_at(15.0).unwrap();
        assert_eq!(seg.character.as_deref(), Some("FIGARO"));
    }

    #[test]
    fn test_json_roundtrip() {
        let libretto = InterchangeLibretto {
            version: "1.0".to_string(),
            opera: InterchangeOpera {
                title: "Tosca".to_string(),
                composer: "Giacomo Puccini".to_string(),
                librettist: None,
                language: "it".to_string(),
                translation_language: Some("en".to_string()),
                year: None,
            },
            tracks: vec![],
        };
        let json = serde_json::to_string_pretty(&libretto).unwrap();
        let parsed: InterchangeLibretto = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.opera.title, "Tosca");
    }
}
