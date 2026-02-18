// Structured score representation.
//
// Transforms flat page descriptions (from LLM analysis of score images)
// into a structured opera score with acts and numbers.

use serde::{Deserialize, Serialize};

use crate::annotate::{PageContentType, PageDescription, PageDescriptions};

/// A structured representation of a vocal score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Score {
    /// Front matter: title, total pages.
    pub front_matter: ScoreFrontMatter,
    /// Musical numbers in score order, each starting at a `continuation=false` page.
    pub numbers: Vec<ScoreNumber>,
}

/// Front matter extracted from the score.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreFrontMatter {
    /// Title from the first page, if available.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Total pages in the score.
    pub total_pages: u32,
    /// Front matter pages (title pages, table of contents, etc.).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub pages: Vec<ScorePage>,
}

/// A single musical number (or section) in the score.
///
/// Each number starts at a page with `continuation=false` and extends
/// through all subsequent `continuation=true` pages until the next
/// `continuation=false` page.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreNumber {
    /// Sequential ID starting at 1.
    pub id: u32,
    /// Content type of the starting page (overture, number, recitative, etc.).
    pub content_type: PageContentType,
    /// Musical form derived from the title (e.g. "duetto", "aria", "recitativo").
    #[serde(skip_serializing_if = "Option::is_none")]
    pub form: Option<String>,
    /// Title/heading from the starting page, if any.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Current act (extracted from titles seen so far).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub act: Option<String>,
    /// All character names seen across pages in this number.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub characters: Vec<String>,
    /// First words from the starting page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_words: Option<String>,
    /// All pages belonging to this number.
    pub pages: Vec<ScorePage>,
}

/// A single page within a score number.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScorePage {
    /// 1-indexed page number.
    pub page: u32,
    /// Content type of this specific page.
    pub content_type: PageContentType,
    /// Characters visible on this page.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub characters: Vec<String>,
    /// First words on this page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_words: Option<String>,
    /// Last words on this page.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_words: Option<String>,
}

/// Build a structured score from page descriptions.
///
/// Groups pages by `continuation=false` boundaries. Each group becomes
/// a `ScoreNumber`. Tracks the current act by scanning titles for
/// "ATTO PRIMO", "ATTO SECONDO", etc.
pub fn build_score(descs: &PageDescriptions) -> Score {
    let mut numbers: Vec<ScoreNumber> = Vec::new();
    let mut current_act: Option<String> = None;
    let mut fm_title: Option<String> = None;
    let mut fm_pages: Vec<ScorePage> = Vec::new();

    for page in &descs.pages {
        // Front matter pages go into the front_matter section
        if page.content_type == PageContentType::FrontMatter {
            if fm_title.is_none() {
                fm_title = page.title.clone();
            }
            fm_pages.push(page_to_score_page(page));
            continue;
        }

        // Check for act changes in the title
        if let Some(ref title) = page.title {
            if let Some(act) = extract_act(title) {
                current_act = Some(act);
            }
        }

        if !page.continuation {
            // Start a new number
            let score_page = page_to_score_page(page);
            let form = page.title.as_deref().and_then(extract_form);
            let id = (numbers.len() + 1) as u32;
            numbers.push(ScoreNumber {
                id,
                content_type: page.content_type.clone(),
                form,
                title: page.title.clone(),
                act: current_act.clone(),
                characters: page.characters.clone(),
                first_words: page.first_words.clone(),
                pages: vec![score_page],
            });
        } else if let Some(current) = numbers.last_mut() {
            // Continuation page — add to current number
            let score_page = page_to_score_page(page);
            current.pages.push(score_page);

            // Accumulate characters
            for c in &page.characters {
                if !current.characters.contains(c) {
                    current.characters.push(c.clone());
                }
            }
        }
    }

    Score {
        front_matter: ScoreFrontMatter {
            title: fm_title,
            total_pages: descs.total_pages,
            pages: fm_pages,
        },
        numbers,
    }
}

fn page_to_score_page(p: &PageDescription) -> ScorePage {
    ScorePage {
        page: p.page,
        content_type: p.content_type.clone(),
        characters: p.characters.clone(),
        first_words: p.first_words.clone(),
        last_words: p.last_words.clone(),
    }
}

/// Extract the musical form from a score title.
///
/// Scans for known Italian form keywords. Compound forms like
/// "RECITATIVO ED ARIA" are detected before their individual parts.
fn extract_form(title: &str) -> Option<String> {
    let t = title.to_uppercase();
    // Compound forms first
    if t.contains("RECITATIVO ED ARIA") { return Some("recitativo ed aria".into()); }
    // Single forms, longest match first where ambiguity is possible
    let forms = [
        "SINFONIA", "FINALE", "SESTETTO", "TERZETTO", "TERZIETTO",
        "DUETTO", "DUETTINO", "CAVATINA", "MARCIA",
        "CORO", "ARIA", "RECITATIVO", "ALLEGRO", "ALLEGRETTO",
        "ANDANTE", "CANZONE",
    ];
    for form in &forms {
        if t.contains(form) {
            return Some(form.to_lowercase());
        }
    }
    None
}

/// Extract an act number from a title string.
///
/// Handles Italian ("ATTO PRIMO") and numeric ("ATTO 1", "ACT 2") forms.
fn extract_act(title: &str) -> Option<String> {
    let t = title.to_uppercase();
    if t.contains("ATTO PRIMO") || t.contains("ACT ONE") || t.contains("ACT 1") {
        return Some("1".to_string());
    }
    if t.contains("ATTO SECONDO") || t.contains("ACT TWO") || t.contains("ACT 2") {
        return Some("2".to_string());
    }
    if t.contains("ATTO TERZO") || t.contains("ACT THREE") || t.contains("ACT 3") {
        return Some("3".to_string());
    }
    if t.contains("ATTO QUARTO") || t.contains("ACT FOUR") || t.contains("ACT 4") {
        return Some("4".to_string());
    }
    if t.contains("ATTO QUINTO") || t.contains("ACT FIVE") || t.contains("ACT 5") {
        return Some("5".to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_page(page: u32, ct: PageContentType, title: Option<&str>,
                 chars: &[&str], first_words: Option<&str>, continuation: bool) -> PageDescription {
        PageDescription {
            page,
            content_type: ct,
            title: title.map(|s| s.to_string()),
            characters: chars.iter().map(|s| s.to_string()).collect(),
            first_words: first_words.map(|s| s.to_string()),
            last_words: None,
            continuation,
        }
    }

    #[test]
    fn test_build_score_basic() {
        let descs = PageDescriptions {
            total_pages: 10,
            pages: vec![
                make_page(1, PageContentType::FrontMatter,
                    Some("Mozart Le nozze di Figaro"), &[], None, false),
                make_page(2, PageContentType::Overture,
                    Some("SINFONIA"), &[], None, false),
                make_page(3, PageContentType::Overture, None, &[], None, true),
                make_page(4, PageContentType::Number,
                    Some("ATTO PRIMO DUETTO"), &["FIGARO"], Some("Cinque"), false),
                make_page(5, PageContentType::Number, None,
                    &["FIGARO", "SUSANNA"], Some("dieci"), true),
                make_page(6, PageContentType::Recitative,
                    Some("RECITATIVO"), &["SUSANNA"], Some("Cosa stai"), false),
            ],
        };

        let score = build_score(&descs);

        assert_eq!(score.front_matter.title.as_deref(), Some("Mozart Le nozze di Figaro"));
        assert_eq!(score.front_matter.total_pages, 10);
        assert_eq!(score.front_matter.pages.len(), 1); // page 1
        assert_eq!(score.front_matter.pages[0].page, 1);
        assert_eq!(score.numbers.len(), 3); // overture, duetto, recitative

        // Overture
        assert_eq!(score.numbers[0].content_type, PageContentType::Overture);
        assert_eq!(score.numbers[0].form.as_deref(), Some("sinfonia"));
        assert_eq!(score.numbers[0].pages.len(), 2); // pages 2-3

        // Duetto — includes SUSANNA accumulated from continuation page
        assert_eq!(score.numbers[1].content_type, PageContentType::Number);
        assert_eq!(score.numbers[1].form.as_deref(), Some("duetto"));
        assert_eq!(score.numbers[1].act.as_deref(), Some("1"));
        assert_eq!(score.numbers[1].characters, vec!["FIGARO", "SUSANNA"]);
        assert_eq!(score.numbers[1].pages.len(), 2); // pages 4-5

        // Recitative
        assert_eq!(score.numbers[2].content_type, PageContentType::Recitative);
        assert_eq!(score.numbers[2].form.as_deref(), Some("recitativo"));
        assert_eq!(score.numbers[2].act.as_deref(), Some("1"));
        assert_eq!(score.numbers[2].pages.len(), 1);
    }

    #[test]
    fn test_act_tracking() {
        let descs = PageDescriptions {
            total_pages: 4,
            pages: vec![
                make_page(1, PageContentType::Number,
                    Some("ATTO PRIMO ARIA"), &["FIGARO"], None, false),
                make_page(2, PageContentType::Number,
                    Some("ATTO SECONDO ARIA"), &["CONTESSA"], None, false),
                make_page(3, PageContentType::Recitative,
                    Some("RECITATIVO"), &["SUSANNA"], None, false),
            ],
        };

        let score = build_score(&descs);

        assert_eq!(score.numbers[0].act.as_deref(), Some("1"));
        assert_eq!(score.numbers[1].act.as_deref(), Some("2"));
        // Recitative inherits Act 2
        assert_eq!(score.numbers[2].act.as_deref(), Some("2"));
    }

    #[test]
    fn test_extract_act() {
        assert_eq!(extract_act("ATTO PRIMO DUETTO"), Some("1".to_string()));
        assert_eq!(extract_act("ATTO SECONDO N° 21"), Some("2".to_string()));
        assert_eq!(extract_act("ATTO TERZO RECITATIVO"), Some("3".to_string()));
        assert_eq!(extract_act("ATTO QUARTO CAVATINA"), Some("4".to_string()));
        assert_eq!(extract_act("RECITATIVO"), None);
        assert_eq!(extract_act("ACT 2"), Some("2".to_string()));
    }
}
