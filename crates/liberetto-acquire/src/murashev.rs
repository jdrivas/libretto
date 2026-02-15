use crate::output;
use crate::types::{AcquiredLibretto, BilingualRow, ContentElement, SourceInfo};
use anyhow::{Context, Result};
use ego_tree;
use scraper::{ElementRef, Html, Node, Selector};
use std::ops::Deref;

const BASE_URL: &str = "https://www.murashev.com/opera";

/// Acquire libretto text from murashev.com.
///
/// Fetches the side-by-side bilingual page, parses the HTML table,
/// extracts pre-aligned paragraph pairs, and writes output files.
///
/// `opera` should be the murashev URL slug (e.g., "Le_nozze_di_Figaro").
/// `lang` should be "en+it", "it+en", "it+de", etc.
pub async fn acquire(opera: &str, lang: &str, output_dir: &str) -> Result<()> {
    let (lang1, lang2) = parse_lang_pair(lang)?;
    let url = build_url(opera, &lang1, &lang2);

    tracing::info!(url = %url, "Fetching from murashev.com");
    let html = fetch_page(&url).await?;
    tracing::info!(bytes = html.len(), "Received HTML");

    let libretto = parse_bilingual_page(&html, &url, opera, &lang1, &lang2)?;
    tracing::info!(rows = libretto.rows.len(), "Parsed bilingual rows");

    output::write_acquired(&libretto, output_dir)?;

    Ok(())
}

/// Parse a language pair string like "en+it" into ("English", "Italian") for URLs
/// and ("en", "it") for codes.
fn parse_lang_pair(lang: &str) -> Result<(LangInfo, LangInfo)> {
    let parts: Vec<&str> = lang.split('+').collect();
    anyhow::ensure!(
        parts.len() == 2,
        "Language pair must be in format 'en+it', got '{lang}'"
    );
    let lang1 = LangInfo::from_code(parts[0])?;
    let lang2 = LangInfo::from_code(parts[1])?;
    Ok((lang1, lang2))
}

#[derive(Debug, Clone)]
struct LangInfo {
    code: String,
    url_name: String,
}

impl LangInfo {
    fn from_code(code: &str) -> Result<Self> {
        let url_name = match code {
            "en" => "English",
            "it" => "Italian",
            "de" => "German",
            "fr" => "French",
            _ => anyhow::bail!("Unsupported language code: {code}"),
        };
        Ok(Self {
            code: code.to_string(),
            url_name: url_name.to_string(),
        })
    }
}

fn build_url(opera: &str, lang1: &LangInfo, lang2: &LangInfo) -> String {
    format!(
        "{BASE_URL}/{opera}_libretto_{}_{}",
        lang1.url_name, lang2.url_name
    )
}

async fn fetch_page(url: &str) -> Result<String> {
    let client = reqwest::Client::builder()
        .user_agent("liberetto/0.1 (opera libretto tool)")
        .build()?;

    let response = client
        .get(url)
        .send()
        .await
        .context("Failed to fetch page")?;

    let status = response.status();
    anyhow::ensure!(status.is_success(), "HTTP {status} for {url}");

    response.text().await.context("Failed to read response body")
}

/// Parse the full HTML page and extract bilingual rows.
fn parse_bilingual_page(
    html: &str,
    url: &str,
    opera: &str,
    lang1: &LangInfo,
    lang2: &LangInfo,
) -> Result<AcquiredLibretto> {
    let document = Html::parse_document(html);

    // Find the bilingual table: table[width="100%"][border="0"][cellspacing="1"]
    let table_sel =
        Selector::parse(r#"table[width="100%"][border="0"][cellspacing="1"]"#)
            .expect("valid selector");
    let table = document
        .select(&table_sel)
        .next()
        .context("Could not find the bilingual table")?;

    let tr_sel = Selector::parse("tr").expect("valid selector");
    let td_sel = Selector::parse("td").expect("valid selector");

    let mut rows = Vec::new();

    for (index, tr) in table.select(&tr_sel).enumerate() {
        let tds: Vec<ElementRef> = tr.select(&td_sel).collect();

        if tds.len() < 2 {
            tracing::debug!(row = index, cols = tds.len(), "Skipping row with < 2 columns");
            continue;
        }

        let lang1_elements = extract_cell_content(tds[0]);
        let lang2_elements = extract_cell_content(tds[1]);

        rows.push(BilingualRow {
            index,
            lang1_elements,
            lang2_elements,
        });
    }

    let now = chrono::Utc::now().to_rfc3339();
    Ok(AcquiredLibretto {
        source: SourceInfo {
            url: url.to_string(),
            site: "murashev.com".to_string(),
            fetched_at: now,
            opera: opera.to_string(),
        },
        lang1: lang1.code.clone(),
        lang2: lang2.code.clone(),
        rows,
    })
}

/// Extract structured content elements from a single `<td>` cell.
///
/// Walks the DOM tree of the cell, recognizing:
/// - `<act>` tags (possibly inside `<span class="act">`) → ActHeader
/// - `<b>` tags → NumberLabel
/// - `<i>` tags → Direction
/// - ALL-CAPS text lines → Character
/// - Other text → Text
/// - Double `<br>` → BlankLine
fn extract_cell_content(td: ElementRef) -> Vec<ContentElement> {
    let mut elements = Vec::new();
    let mut pending_text = String::new();
    let mut consecutive_br = 0;

    fn flush_text(pending: &mut String, elements: &mut Vec<ContentElement>) {
        let trimmed = pending.trim();
        if !trimmed.is_empty() {
            // Classify the line
            if is_character_name(trimmed) {
                elements.push(ContentElement::Character(trimmed.to_string()));
            } else {
                elements.push(ContentElement::Text(trimmed.to_string()));
            }
        }
        pending.clear();
    }

    walk_node(
        td.id(),
        td.tree(),
        &mut elements,
        &mut pending_text,
        &mut consecutive_br,
    );

    // Flush any remaining text
    flush_text(&mut pending_text, &mut elements);

    elements
}

fn walk_node(
    node_id: ego_tree::NodeId,
    tree: &ego_tree::Tree<Node>,
    elements: &mut Vec<ContentElement>,
    pending_text: &mut String,
    consecutive_br: &mut u32,
) {
    let node = tree.get(node_id).expect("valid node id");

    match node.value() {
        Node::Text(text) => {
            *consecutive_br = 0;
            pending_text.push_str(text.deref());
        }
        Node::Element(elem) => {
            let tag = elem.name();
            match tag {
                "br" => {
                    *consecutive_br += 1;
                    // Flush current text as a line
                    let trimmed = pending_text.trim().to_string();
                    if !trimmed.is_empty() {
                        if is_character_name(&trimmed) {
                            elements.push(ContentElement::Character(trimmed));
                        } else {
                            elements.push(ContentElement::Text(trimmed));
                        }
                    }
                    pending_text.clear();

                    if *consecutive_br >= 2 {
                        elements.push(ContentElement::BlankLine);
                        *consecutive_br = 0;
                    }
                }
                "act" => {
                    // Custom <act> tag — extract inner text as act header
                    let text = collect_all_text(node_id, tree);
                    let trimmed = text.trim().to_string();
                    if !trimmed.is_empty() {
                        // Flush any pending text first
                        let pt = pending_text.trim().to_string();
                        if !pt.is_empty() {
                            if is_character_name(&pt) {
                                elements.push(ContentElement::Character(pt));
                            } else {
                                elements.push(ContentElement::Text(pt));
                            }
                            pending_text.clear();
                        }
                        elements.push(ContentElement::ActHeader(trimmed));
                    }
                    return; // Don't recurse into children, we already collected text
                }
                "b" => {
                    let text = collect_all_text(node_id, tree);
                    let trimmed = text.trim().to_string();
                    if !trimmed.is_empty() {
                        let pt = pending_text.trim().to_string();
                        if !pt.is_empty() {
                            if is_character_name(&pt) {
                                elements.push(ContentElement::Character(pt));
                            } else {
                                elements.push(ContentElement::Text(pt));
                            }
                            pending_text.clear();
                        }
                        elements.push(ContentElement::NumberLabel(trimmed));
                    }
                    return;
                }
                "i" => {
                    let text = collect_all_text(node_id, tree);
                    let trimmed = text.trim().to_string();
                    if !trimmed.is_empty() {
                        let pt = pending_text.trim().to_string();
                        if !pt.is_empty() {
                            if is_character_name(&pt) {
                                elements.push(ContentElement::Character(pt));
                            } else {
                                elements.push(ContentElement::Text(pt));
                            }
                            pending_text.clear();
                        }
                        elements.push(ContentElement::Direction(trimmed));
                    }
                    return;
                }
                "span" => {
                    // <span class="act"> wraps <act>, just recurse
                    for child in node.children() {
                        walk_node(child.id(), tree, elements, pending_text, consecutive_br);
                    }
                    return;
                }
                "td" | "div" | "p" | "a" => {
                    // Container elements — recurse into children
                    for child in node.children() {
                        walk_node(child.id(), tree, elements, pending_text, consecutive_br);
                    }
                    return;
                }
                _ => {
                    // Unknown element — collect its text content
                    for child in node.children() {
                        walk_node(child.id(), tree, elements, pending_text, consecutive_br);
                    }
                    return;
                }
            }
        }
        _ => {}
    }
}

/// Collect all text content under a node, recursively.
fn collect_all_text(node_id: ego_tree::NodeId, tree: &ego_tree::Tree<Node>) -> String {
    let node = tree.get(node_id).expect("valid node id");
    let mut text = String::new();

    for child in node.children() {
        match child.value() {
            Node::Text(t) => text.push_str(t.deref()),
            Node::Element(elem) => {
                if elem.name() == "br" {
                    text.push('\n');
                } else {
                    text.push_str(&collect_all_text(child.id(), tree));
                }
            }
            _ => {}
        }
    }

    text
}

/// Heuristic: a line is a character name if it's mostly uppercase letters,
/// possibly with commas, spaces, and parenthesized stage directions.
fn is_character_name(s: &str) -> bool {
    // Strip any parenthesized suffix like "(making a curtsy)"
    let base = if let Some(idx) = s.find('(') {
        s[..idx].trim()
    } else {
        s.trim()
    };

    if base.is_empty() {
        return false;
    }

    // Must have at least 2 uppercase letters
    let upper_count = base.chars().filter(|c| c.is_uppercase()).count();
    if upper_count < 2 {
        return false;
    }

    // All alphabetic characters should be uppercase
    let alpha_chars: Vec<char> = base.chars().filter(|c| c.is_alphabetic()).collect();
    if alpha_chars.is_empty() {
        return false;
    }

    if !alpha_chars.iter().all(|c| c.is_uppercase()) {
        return false;
    }

    // Exclude common act/section header patterns that are also all-caps.
    // These are normally caught by the <act> tag, but guard against
    // edge cases where they appear as plain text.
    let upper_base = base.to_uppercase();
    let act_patterns = [
        "ACT ", "ATTO ", "ACTE ", "AKT ",
        "OVERTURE", "SINFONIA", "OUVERTURE",
        "END OF", "FIN ",
    ];
    if act_patterns.iter().any(|p| upper_base.starts_with(p)) {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_character_name() {
        assert!(is_character_name("FIGARO"));
        assert!(is_character_name("SUSANNA"));
        assert!(is_character_name("SUSANNA, FIGARO"));
        assert!(is_character_name("IL CONTE"));
        assert!(is_character_name("MARCELLINA (making a curtsy)"));

        assert!(!is_character_name("If you would dance,"));
        assert!(!is_character_name("No. 1: Duettino"));
        assert!(!is_character_name("ACT ONE"));
        assert!(!is_character_name("ATTO PRIMO"));
        assert!(!is_character_name("OVERTURE"));
        assert!(!is_character_name("END OF THE OPERA"));
        assert!(!is_character_name("a"));
        assert!(!is_character_name(""));
    }

    #[test]
    fn test_parse_lang_pair() {
        let (l1, l2) = parse_lang_pair("en+it").unwrap();
        assert_eq!(l1.code, "en");
        assert_eq!(l2.code, "it");
        assert_eq!(l1.url_name, "English");
        assert_eq!(l2.url_name, "Italian");
    }

    #[test]
    fn test_build_url() {
        let l1 = LangInfo { code: "en".into(), url_name: "English".into() };
        let l2 = LangInfo { code: "it".into(), url_name: "Italian".into() };
        let url = build_url("Le_nozze_di_Figaro", &l1, &l2);
        assert_eq!(
            url,
            "https://www.murashev.com/opera/Le_nozze_di_Figaro_libretto_English_Italian"
        );
    }

    #[test]
    fn test_parse_bilingual_table() {
        let html = r#"
        <html><body>
        <table width="100%" border="0" cellspacing="1" cellpadding="5">
          <tr>
            <td width="50%" valign="top">
              <span class="act"><act>ACT ONE</act><br /></span>
              <b>No. 1: Duettino</b><br />
              FIGARO<br />
              Five... ten... twenty...<br />
            </td>
            <td width="50%" valign="top">
              <span class="act"><act>ATTO PRIMO</act><br /></span>
              <b>N° 1: Duettino</b><br />
              FIGARO<br />
              Cinque... dieci... venti...<br />
            </td>
          </tr>
          <tr>
            <td width="50%" valign="top">
              SUSANNA<br />
              <i>(looking at herself in a mirror)</i><br />
              How happy I am now.<br />
            </td>
            <td width="50%" valign="top">
              SUSANNA<br />
              <i>(guardandosi nello specchio)</i><br />
              Ora sì ch'io son contenta.<br />
            </td>
          </tr>
        </table>
        </body></html>
        "#;

        let l1 = LangInfo { code: "en".into(), url_name: "English".into() };
        let l2 = LangInfo { code: "it".into(), url_name: "Italian".into() };

        let libretto = parse_bilingual_page(
            html,
            "https://test.example.com",
            "Le_nozze_di_Figaro",
            &l1,
            &l2,
        )
        .unwrap();

        assert_eq!(libretto.rows.len(), 2);

        // Row 0: Act header + number label + character + text
        let row0 = &libretto.rows[0];
        assert!(row0.lang1_elements.contains(&ContentElement::ActHeader("ACT ONE".into())));
        assert!(row0.lang1_elements.contains(&ContentElement::NumberLabel("No. 1: Duettino".into())));
        assert!(row0.lang1_elements.contains(&ContentElement::Character("FIGARO".into())));
        assert!(row0.lang1_elements.contains(&ContentElement::Text("Five... ten... twenty...".into())));

        assert!(row0.lang2_elements.contains(&ContentElement::ActHeader("ATTO PRIMO".into())));
        assert!(row0.lang2_elements.contains(&ContentElement::NumberLabel("N° 1: Duettino".into())));

        // Row 1: Character + direction + text
        let row1 = &libretto.rows[1];
        assert!(row1.lang1_elements.contains(&ContentElement::Character("SUSANNA".into())));
        assert!(row1.lang1_elements.contains(&ContentElement::Direction("(looking at herself in a mirror)".into())));
        assert!(row1.lang1_elements.contains(&ContentElement::Text("How happy I am now.".into())));

        assert!(row1.lang2_elements.contains(&ContentElement::Character("SUSANNA".into())));
        assert!(row1.lang2_elements.contains(&ContentElement::Direction("(guardandosi nello specchio)".into())));
    }
}
