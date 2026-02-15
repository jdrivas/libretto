use crate::normalize;
use crate::types::{AcquiredLibretto, AcquiredMonolingual, ContentElement, SourceInfo};
use anyhow::Result;
use std::fs;
use std::path::Path;

/// Write all bilingual acquisition output files to the given directory.
///
/// Creates the directory if it doesn't exist, then writes:
/// - `{lang1}.txt` (e.g., `english.txt`) — human convenience
/// - `{lang2}.txt` (e.g., `italian.txt`) — human convenience
/// - `bilingual.json` — structured pre-aligned pairs (parser input)
/// - `source.md` — provenance info
pub fn write_acquired(libretto: &AcquiredLibretto, output_dir: &str) -> Result<()> {
    let dir = Path::new(output_dir);
    fs::create_dir_all(dir)?;

    let lang1_name = lang_code_to_name(&libretto.lang1);
    let lang2_name = lang_code_to_name(&libretto.lang2);

    // Write plain text files (human convenience)
    let lang1_text = normalize::normalize_text(&libretto.lang1_text());
    let lang1_text = normalize::collapse_blank_lines(&lang1_text);
    fs::write(dir.join(format!("{lang1_name}.txt")), &lang1_text)?;
    tracing::info!(path = %dir.join(format!("{lang1_name}.txt")).display(), lines = lang1_text.lines().count(), "Wrote {lang1_name} text");

    let lang2_text = normalize::normalize_text(&libretto.lang2_text());
    let lang2_text = normalize::collapse_blank_lines(&lang2_text);
    fs::write(dir.join(format!("{lang2_name}.txt")), &lang2_text)?;
    tracing::info!(path = %dir.join(format!("{lang2_name}.txt")).display(), lines = lang2_text.lines().count(), "Wrote {lang2_name} text");

    // Write bilingual JSON (parser input — source of truth)
    let json = serde_json::to_string_pretty(libretto)?;
    fs::write(dir.join("bilingual.json"), &json)?;
    tracing::info!(path = %dir.join("bilingual.json").display(), rows = libretto.rows.len(), "Wrote bilingual JSON");

    // Write source provenance
    fs::write(dir.join("source.md"), libretto.source_md())?;
    tracing::info!(path = %dir.join("source.md").display(), "Wrote source provenance");

    Ok(())
}

/// Write single-language acquisition output files to the given directory.
///
/// Creates the directory if it doesn't exist, then writes:
/// - `{lang}.txt` (e.g., `english.txt`) — human convenience
/// - `monolingual.json` — structured typed elements (parser input)
/// - `source.md` — provenance info
pub fn write_single_language(
    elements: &[ContentElement],
    lang: &str,
    url: &str,
    site: &str,
    opera: &str,
    output_dir: &str,
) -> Result<()> {
    let dir = Path::new(output_dir);
    fs::create_dir_all(dir)?;

    let lang_name = lang_code_to_name(lang);
    let now = chrono::Utc::now().to_rfc3339();

    // Build the structured monolingual representation
    let acquired = AcquiredMonolingual {
        source: SourceInfo {
            url: url.to_string(),
            site: site.to_string(),
            fetched_at: now,
            opera: opera.to_string(),
        },
        lang: lang.to_string(),
        elements: elements.to_vec(),
    };

    // Write monolingual JSON (parser input — source of truth)
    let json_filename = format!("{lang_name}.json");
    let json = serde_json::to_string_pretty(&acquired)?;
    fs::write(dir.join(&json_filename), &json)?;
    tracing::info!(path = %dir.join(&json_filename).display(), elements = acquired.elements.len(), "Wrote monolingual JSON");

    // Write plain text file (human convenience)
    let text = acquired.plain_text();
    let text = normalize::normalize_text(&text);
    let text = normalize::collapse_blank_lines(&text);
    let path = dir.join(format!("{lang_name}.txt"));
    fs::write(&path, &text)?;
    tracing::info!(path = %path.display(), lines = text.lines().count(), "Wrote {lang_name} text");

    // Write source provenance
    fs::write(dir.join("source.md"), acquired.source_md())?;
    tracing::info!("Wrote source provenance");

    Ok(())
}

/// Cache raw HTML to the output directory for archival/debugging.
///
/// Writes one or more HTML files so the original page can be re-examined
/// without re-fetching.
pub fn cache_html(output_dir: &str, filename: &str, html: &str) -> Result<()> {
    let dir = Path::new(output_dir);
    fs::create_dir_all(dir)?;
    let path = dir.join(filename);
    fs::write(&path, html)?;
    tracing::info!(path = %path.display(), bytes = html.len(), "Cached raw HTML");
    Ok(())
}

fn lang_code_to_name(code: &str) -> &str {
    match code {
        "it" => "italian",
        "en" => "english",
        "de" => "german",
        "fr" => "french",
        "es" => "spanish",
        "ru" => "russian",
        other => other,
    }
}
