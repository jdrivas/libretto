use serde::{Deserialize, Serialize};

/// A complete acquired bilingual libretto before parsing into BaseLibretto.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AcquiredLibretto {
    pub source: SourceInfo,
    /// ISO 639-1 code for language in column 1 (e.g., "en").
    pub lang1: String,
    /// ISO 639-1 code for language in column 2 (e.g., "it").
    pub lang2: String,
    /// Pre-aligned bilingual rows extracted from the source.
    pub rows: Vec<BilingualRow>,
}

/// Provenance information about the acquisition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceInfo {
    pub url: String,
    pub site: String,
    pub fetched_at: String,
    pub opera: String,
}

/// A single row from a bilingual table: one paragraph in two languages.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BilingualRow {
    pub index: usize,
    pub lang1_elements: Vec<ContentElement>,
    pub lang2_elements: Vec<ContentElement>,
}

/// A structural element extracted from an HTML cell.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", content = "text")]
pub enum ContentElement {
    /// Act/section header (e.g., "ACT ONE", "ATTO PRIMO").
    ActHeader(String),
    /// Musical number label (e.g., "No. 1: Duettino", "N° 1: Duettino").
    NumberLabel(String),
    /// Character name in ALL CAPS (e.g., "FIGARO", "SUSANNA, FIGARO").
    Character(String),
    /// Stage direction in italics (e.g., "(Figaro is measuring the floor.)").
    Direction(String),
    /// Sung or spoken text.
    Text(String),
    /// A blank line separating stanzas or sections.
    BlankLine,
}

impl BilingualRow {
    /// Extract plain text from one language column, collapsing elements into lines.
    pub fn plain_text(elements: &[ContentElement]) -> String {
        let mut lines = Vec::new();
        for elem in elements {
            match elem {
                ContentElement::ActHeader(s) => lines.push(s.clone()),
                ContentElement::NumberLabel(s) => lines.push(s.clone()),
                ContentElement::Character(s) => lines.push(s.clone()),
                ContentElement::Direction(s) => lines.push(s.clone()),
                ContentElement::Text(s) => lines.push(s.clone()),
                ContentElement::BlankLine => lines.push(String::new()),
            }
        }
        lines.join("\n")
    }
}

impl AcquiredLibretto {
    /// Generate the full plain text for language 1.
    pub fn lang1_text(&self) -> String {
        self.rows
            .iter()
            .map(|r| BilingualRow::plain_text(&r.lang1_elements))
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// Generate the full plain text for language 2.
    pub fn lang2_text(&self) -> String {
        self.rows
            .iter()
            .map(|r| BilingualRow::plain_text(&r.lang2_elements))
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    /// Generate a source.md provenance file.
    pub fn source_md(&self) -> String {
        format!(
            "# Source\n\n\
             - **Site:** {}\n\
             - **URL:** {}\n\
             - **Opera:** {}\n\
             - **Fetched:** {}\n\
             - **Languages:** {} + {}\n\
             - **Rows:** {}\n",
            self.source.site,
            self.source.url,
            self.source.opera,
            self.source.fetched_at,
            self.lang1,
            self.lang2,
            self.rows.len(),
        )
    }
}
