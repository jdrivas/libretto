use crate::normalize;
use crate::types::AcquiredLibretto;
use anyhow::Result;
use std::fs;
use std::path::Path;

/// Write all acquisition output files to the given directory.
///
/// Creates the directory if it doesn't exist, then writes:
/// - `{lang1}.txt` (e.g., `english.txt`)
/// - `{lang2}.txt` (e.g., `italian.txt`)
/// - `bilingual.json` — structured pre-aligned pairs
/// - `source.md` — provenance info
pub fn write_acquired(libretto: &AcquiredLibretto, output_dir: &str) -> Result<()> {
    let dir = Path::new(output_dir);
    fs::create_dir_all(dir)?;

    let lang1_name = lang_code_to_name(&libretto.lang1);
    let lang2_name = lang_code_to_name(&libretto.lang2);

    // Write plain text files
    let lang1_text = normalize::normalize_text(&libretto.lang1_text());
    let lang1_text = normalize::collapse_blank_lines(&lang1_text);
    fs::write(dir.join(format!("{lang1_name}.txt")), &lang1_text)?;
    tracing::info!(path = %dir.join(format!("{lang1_name}.txt")).display(), lines = lang1_text.lines().count(), "Wrote {lang1_name} text");

    let lang2_text = normalize::normalize_text(&libretto.lang2_text());
    let lang2_text = normalize::collapse_blank_lines(&lang2_text);
    fs::write(dir.join(format!("{lang2_name}.txt")), &lang2_text)?;
    tracing::info!(path = %dir.join(format!("{lang2_name}.txt")).display(), lines = lang2_text.lines().count(), "Wrote {lang2_name} text");

    // Write bilingual JSON
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
/// - `{lang}.txt` (e.g., `english.txt`)
/// - `source.md` — provenance info
pub fn write_single_language(
    elements: &[crate::types::ContentElement],
    lang: &str,
    url: &str,
    opera: &str,
    output_dir: &str,
) -> Result<()> {
    let dir = Path::new(output_dir);
    fs::create_dir_all(dir)?;

    let lang_name = lang_code_to_name(lang);

    // Convert elements to plain text
    let text = crate::types::BilingualRow::plain_text(elements);
    let text = normalize::normalize_text(&text);
    let text = normalize::collapse_blank_lines(&text);
    let path = dir.join(format!("{lang_name}.txt"));
    fs::write(&path, &text)?;
    tracing::info!(path = %path.display(), lines = text.lines().count(), "Wrote {lang_name} text");

    // Write source provenance
    let source_md = format!(
        "# Source\n\n\
         - **Site:** murashev.com\n\
         - **URL:** {url}\n\
         - **Opera:** {opera}\n\
         - **Fetched:** {}\n\
         - **Language:** {lang}\n",
        chrono::Utc::now().to_rfc3339(),
    );
    fs::write(dir.join("source.md"), &source_md)?;
    tracing::info!("Wrote source provenance");

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
