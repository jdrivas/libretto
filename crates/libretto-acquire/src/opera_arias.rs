use crate::output;
use crate::types::ContentElement;
use anyhow::{Context, Result};
use ego_tree;
use scraper::{Html, Node, Selector};
use std::ops::Deref;

const BASE_URL: &str = "https://www.opera-arias.com";

/// Acquire libretto text from opera-arias.com.
///
/// Fetches the Italian and/or English libretto pages, parses the HTML,
/// extracts the libretto text, and writes structured JSON + plain text files.
///
/// `opera` should be the opera-arias.com path slug (e.g., "mozart/le-nozze-di-figaro").
/// `lang` should be comma-separated: "it", "en", or "it,en".
pub async fn acquire(opera: &str, lang: &str, output_dir: &str) -> Result<()> {
    let langs: Vec<&str> = lang.split(',').map(|s| s.trim()).collect();

    for lang_code in &langs {
        let (url, div_class) = match *lang_code {
            "it" => (
                format!("{BASE_URL}/{opera}/libretto/"),
                "libretto_div",
            ),
            "en" => (
                format!("{BASE_URL}/{opera}/libretto/english/"),
                "translation_div",
            ),
            other => anyhow::bail!("Unsupported language for opera-arias.com: {other}"),
        };

        tracing::info!(url = %url, lang = lang_code, "Fetching from opera-arias.com");
        let html = fetch_page(&url).await?;
        tracing::info!(bytes = html.len(), "Received HTML");

        // Cache raw HTML
        let html_filename = format!("raw_{}.html", lang_code);
        output::cache_html(output_dir, &html_filename, &html)?;

        let elements = parse_libretto_page(&html, div_class)?;
        tracing::info!(elements = elements.len(), lang = lang_code, "Parsed content elements");

        // Write structured JSON + plain text + source.md via shared output helper
        output::write_single_language(&elements, lang_code, &url, "opera-arias.com", opera, output_dir)?;
    }

    Ok(())
}

async fn fetch_page(url: &str) -> Result<String> {
    let client = reqwest::Client::builder()
        .user_agent("libretto/0.1 (opera libretto tool)")
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

/// Parse the libretto/translation page and extract content elements.
fn parse_libretto_page(html: &str, div_class: &str) -> Result<Vec<ContentElement>> {
    let document = Html::parse_document(html);

    let selector_str = format!("div.{div_class}");
    let div_sel = Selector::parse(&selector_str).expect("valid selector");

    let content_div = document
        .select(&div_sel)
        .next()
        .with_context(|| format!("Could not find div.{div_class}"))?;

    let mut elements = Vec::new();
    let mut pending_text = String::new();
    let mut consecutive_br = 0;

    walk_node(
        content_div.id(),
        content_div.tree(),
        &mut elements,
        &mut pending_text,
        &mut consecutive_br,
    );

    // Flush remaining
    flush_text(&mut pending_text, &mut elements);

    Ok(elements)
}

fn flush_text(pending: &mut String, elements: &mut Vec<ContentElement>) {
    let trimmed = pending.trim();
    if !trimmed.is_empty() {
        if is_character_name(trimmed) {
            elements.push(ContentElement::Character(trimmed.to_string()));
        } else {
            elements.push(ContentElement::Text(trimmed.to_string()));
        }
    }
    pending.clear();
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
                "hr" => {
                    flush_text(pending_text, elements);
                    elements.push(ContentElement::BlankLine);
                    *consecutive_br = 0;
                }
                "b" => {
                    let text = collect_all_text(node_id, tree);
                    let trimmed = text.trim().to_string();
                    if !trimmed.is_empty() {
                        flush_text(pending_text, elements);
                        // Classify bold text: act headers vs number labels vs other
                        if is_act_header(&trimmed) {
                            elements.push(ContentElement::ActHeader(trimmed));
                        } else {
                            elements.push(ContentElement::NumberLabel(trimmed));
                        }
                    }
                    return;
                }
                "i" => {
                    let text = collect_all_text(node_id, tree);
                    let trimmed = text.trim().to_string();
                    if !trimmed.is_empty() {
                        flush_text(pending_text, elements);
                        elements.push(ContentElement::Direction(trimmed));
                    }
                    return;
                }
                "h1" | "h2" => {
                    // Skip title headers — they're page chrome, not libretto text
                    return;
                }
                "script" | "ins" | "style" => {
                    // Skip ad/script elements
                    return;
                }
                _ => {
                    // Container elements (div, p, span, a, etc.) — recurse
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

/// Heuristic: detect act/section headers in bold text.
fn is_act_header(s: &str) -> bool {
    let upper = s.to_uppercase();
    let patterns = [
        "ATTO ", "ACT ", "ACTE ", "AKT ",
        "OVERTURE", "OUVERTURE", "SINFONIA",
        "PERSONAGGI", "CAST",
    ];
    patterns.iter().any(|p| upper.starts_with(p))
}

/// Heuristic: a line is a character name if it's all uppercase letters
/// (with spaces, commas, and possible parenthesized directions).
fn is_character_name(s: &str) -> bool {
    let base = if let Some(idx) = s.find('(') {
        s[..idx].trim()
    } else {
        s.trim()
    };

    if base.is_empty() {
        return false;
    }

    let upper_count = base.chars().filter(|c| c.is_uppercase()).count();
    if upper_count < 2 {
        return false;
    }

    // Split on whitespace and check: allow lowercase connector words (e, and, et, di)
    let words: Vec<&str> = base.split_whitespace().collect();
    let connectors = ["e", "and", "et", "di", "de", "la", "il"];
    for word in &words {
        // Strip punctuation for check
        let clean: String = word.chars().filter(|c| c.is_alphabetic()).collect();
        if clean.is_empty() {
            continue;
        }
        if connectors.contains(&clean.as_str()) {
            continue;
        }
        if !clean.chars().all(|c| c.is_uppercase()) {
            return false;
        }
    }

    // Exclude act/section headers
    let upper_base = base.to_uppercase();
    let act_patterns = [
        "ACT ", "ATTO ", "ACTE ", "AKT ",
        "OVERTURE", "SINFONIA", "OUVERTURE",
        "END OF", "FIN ", "SCENA", "SCENE",
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
    fn test_parse_libretto_div() {
        let html = r#"
        <html><body>
        <div class="libretto_div">
            <h1>Test Opera Libretto</h1>
            <p>
            <b>Personaggi:</b><br>
            FIGARO, camariere del conte (Basso)<br>
            <br>
            CORO</p>
            <hr>
            <br>
            <p>
            <b>ATTO PRIMO</b><br>
            <br>
            <i>Camera non affatto ammobiliata</i><br>
            <br>
            <b>No. 1 - Duettino</b><br>
            <br>
            FIGARO<br>
            <i>misurando</i><br>
            Cinque... dieci... venti...<br>
            </p>
        </div>
        </body></html>
        "#;

        let elements = parse_libretto_page(html, "libretto_div").unwrap();

        assert!(elements.contains(&ContentElement::ActHeader("Personaggi:".into())));
        assert!(elements.contains(&ContentElement::ActHeader("ATTO PRIMO".into())));
        assert!(elements.contains(&ContentElement::Direction("Camera non affatto ammobiliata".into())));
        assert!(elements.contains(&ContentElement::NumberLabel("No. 1 - Duettino".into())));
        assert!(elements.contains(&ContentElement::Character("FIGARO".into())));
        assert!(elements.contains(&ContentElement::Direction("misurando".into())));
        assert!(elements.contains(&ContentElement::Text("Cinque... dieci... venti...".into())));
    }

    #[test]
    fn test_parse_translation_div() {
        let html = r#"
        <html><body>
        <div class="translation_div">
            <h1>Test Opera English Translation</h1>
            <p>
            <b>ACT ONE</b><br>
            <br>
            <b>Duettino</b><br>
            <br>
            FIGARO<br>
            Five ... ten ... twenty ...<br>
            </p>
        </div>
        </body></html>
        "#;

        let elements = parse_libretto_page(html, "translation_div").unwrap();

        assert!(elements.contains(&ContentElement::ActHeader("ACT ONE".into())));
        assert!(elements.contains(&ContentElement::NumberLabel("Duettino".into())));
        assert!(elements.contains(&ContentElement::Character("FIGARO".into())));
        assert!(elements.contains(&ContentElement::Text("Five ... ten ... twenty ...".into())));
    }

    #[test]
    fn test_is_act_header() {
        assert!(is_act_header("ATTO PRIMO"));
        assert!(is_act_header("ACT ONE"));
        assert!(is_act_header("Overture"));
        assert!(is_act_header("Personaggi:"));
        assert!(!is_act_header("No. 1 - Duettino"));
        assert!(!is_act_header("Recitativo"));
    }

    #[test]
    fn test_is_character_name() {
        assert!(is_character_name("FIGARO"));
        assert!(is_character_name("SUSANNA e FIGARO"));
        assert!(is_character_name("IL CONTE"));
        assert!(is_character_name("SUSANNA, LA CONTESSA"));
        assert!(!is_character_name("SCENE ONE"));
        assert!(!is_character_name("SCENA I"));
        assert!(!is_character_name("Five ... ten ..."));
    }
}
