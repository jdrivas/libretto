// LLM-assisted annotation generation.
//
// Extracts pages from a vocal score PDF as images, sends them to Claude
// along with the segment list for a musical number, and parses the
// structured response into a NumberAnnotation.

use std::path::{Path, PathBuf};

use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::base_libretto::BaseLibretto;
use super::{NumberAnnotation, GroupAnnotation};

/// Configuration for LLM-assisted annotation generation.
#[derive(Debug, Clone)]
pub struct GenerateConfig {
    /// Directory containing pre-ingested page images (p001.png, p002.png, ...).
    /// If set, pages are loaded from here instead of extracting from PDF.
    pub pages_dir: Option<PathBuf>,
    /// Path to the vocal score PDF (used if pages_dir is None).
    pub score_path: Option<PathBuf>,
    /// Anthropic API key.
    pub api_key: String,
    /// Model to use (default: "claude-sonnet-4-20250514").
    pub model: String,
    /// Temporary directory for extracted page images (when using PDF directly).
    pub temp_dir: PathBuf,
}

/// Extract specific pages from a PDF as PNG images using mupdf.
///
/// Pages are 1-indexed (matching PDF page numbers).
/// Returns paths to the extracted PNG files.
pub fn extract_pdf_pages(
    pdf_path: &Path,
    pages: &[u32],
    output_dir: &Path,
) -> Result<Vec<PathBuf>, String> {
    std::fs::create_dir_all(output_dir)
        .map_err(|e| format!("Failed to create output dir: {e}"))?;

    let doc = mupdf::Document::open(pdf_path.to_str().unwrap_or(""))
        .map_err(|e| format!("Failed to open PDF: {e}"))?;

    let mut paths = Vec::new();
    // Scale factor: 150 DPI / 72 points-per-inch = ~2.08×
    let scale = 150.0 / 72.0;
    let matrix = mupdf::Matrix::new_scale(scale, scale);

    for &page_num in pages {
        let page = doc.load_page((page_num as i32) - 1)
            .map_err(|e| format!("Failed to load page {page_num}: {e}"))?;

        let pixmap = page.to_pixmap(&matrix, &mupdf::Colorspace::device_rgb(), false, false)
            .map_err(|e| format!("Failed to render page {page_num}: {e}"))?;

        let out_path = output_dir.join(format!("p{page_num:03}.png"));
        pixmap.save_as(out_path.to_str().unwrap_or(""), mupdf::pixmap::ImageFormat::PNG)
            .map_err(|e| format!("Failed to save page {page_num} as PNG: {e}"))?;

        paths.push(out_path);
    }

    Ok(paths)
}

/// Build the prompt for Claude to annotate a musical number.
fn build_prompt(base: &BaseLibretto, number_id: &str) -> Result<String, String> {
    let number = base.find_number(number_id)
        .ok_or_else(|| format!("Number '{}' not found in base libretto", number_id))?;

    let mut segment_list = String::new();
    for seg in &number.segments {
        let char_str = seg.character.as_deref().unwrap_or("---");
        let text_str = seg.text.as_deref().unwrap_or(
            seg.direction.as_deref().unwrap_or("[no text]")
        );
        let stype = format!("{:?}", seg.segment_type).to_lowercase();
        segment_list.push_str(&format!(
            "  {} | {} | {} | {}\n",
            seg.id, stype, char_str, text_str
        ));
    }

    Ok(format!(
        r#"You are analyzing vocal score pages for the musical number "{label}" ({number_id}).

Here are the libretto segments for this number:

  SEGMENT_ID | TYPE | CHARACTER | TEXT
{segments}
The score pages for this number are attached as images.

Please analyze the score and provide the following as a JSON object:

1. **tempo_marking**: The tempo marking at the start of this number (e.g., "Allegro", "Andante con moto"). Use the exact Italian text from the score. Set to null if not visible.

2. **groups**: Identify segments that are sung SIMULTANEOUSLY (multiple voices singing at the same time with different text). For each group of simultaneously-sung segments, provide a group tag (use sequential letters: "a", "b", "c", etc.) and list the segment IDs.

Rules for groups:
- Only group segments that genuinely overlap in the score (multiple vocal staves active at the same time with different lyrics)
- Segments where characters sing the SAME text together (marked with multiple character names like "SUSANNA, FIGARO") do NOT need grouping
- Sequential dialogue (one character after another) does NOT need grouping
- Stage directions and interludes never get groups

Respond with ONLY a JSON object in this exact format (no markdown, no explanation):
{{
  "tempo_marking": "Allegro" or null,
  "groups": [
    {{
      "group": "a",
      "segment_ids": ["seg-001", "seg-002"]
    }}
  ]
}}
"#,
        label = number.label,
        number_id = number_id,
        segments = segment_list,
    ))
}

/// Response structure from Claude API.
#[derive(Debug, Deserialize)]
struct ClaudeResponse {
    content: Vec<ContentBlock>,
}

#[derive(Debug, Deserialize)]
struct ContentBlock {
    text: Option<String>,
}

/// Parsed LLM response.
#[derive(Debug, Deserialize, Serialize)]
struct LlmAnnotationResponse {
    tempo_marking: Option<String>,
    groups: Vec<LlmGroup>,
}

#[derive(Debug, Deserialize, Serialize)]
struct LlmGroup {
    group: String,
    segment_ids: Vec<String>,
}

/// Extract a JSON object from a response that may contain surrounding text.
///
/// Handles: bare JSON, markdown code fences, and explanation text before/after.
fn extract_json_from_response(text: &str) -> Option<String> {
    let trimmed = text.trim();

    // Try 1: markdown code fence
    if let Some(start) = trimmed.find("```json") {
        let after = &trimmed[start + 7..];
        if let Some(end) = after.find("```") {
            return Some(after[..end].trim().to_string());
        }
    }
    if let Some(start) = trimmed.find("```\n{") {
        let after = &trimmed[start + 3..];
        if let Some(end) = after.find("```") {
            return Some(after[..end].trim().to_string());
        }
    }

    // Try 2: find the outermost [...] or {...} by bracket counting.
    // Prefer arrays (for score map responses) over objects.
    for (open, close) in [('[', ']'), ('{', '}')] {
        let mut depth = 0i32;
        let mut start_idx = None;
        for (i, ch) in trimmed.char_indices() {
            if ch == open {
                if depth == 0 {
                    start_idx = Some(i);
                }
                depth += 1;
            } else if ch == close {
                depth -= 1;
                if depth == 0 {
                    if let Some(s) = start_idx {
                        let candidate = &trimmed[s..=i];
                        if serde_json::from_str::<serde_json::Value>(candidate).is_ok() {
                            return Some(candidate.to_string());
                        }
                    }
                    start_idx = None;
                }
            }
        }
    }

    // Try 3: the whole thing is JSON
    if serde_json::from_str::<serde_json::Value>(trimmed).is_ok() {
        return Some(trimmed.to_string());
    }

    None
}

/// Resolve page image paths: use pre-ingested directory or extract from PDF.
fn resolve_page_images(
    config: &GenerateConfig,
    pages: &[u32],
    label: &str,
) -> Result<(Vec<PathBuf>, bool), String> {
    if let Some(ref pages_dir) = config.pages_dir {
        // Use pre-ingested images
        let paths: Vec<PathBuf> = pages.iter()
            .map(|&p| pages_dir.join(format!("p{p:03}.png")))
            .collect();
        // Verify they exist
        for p in &paths {
            if !p.exists() {
                return Err(format!("Page image not found: {}", p.display()));
            }
        }
        Ok((paths, false)) // false = don't clean up
    } else if let Some(ref score_path) = config.score_path {
        let page_dir = config.temp_dir.join(label);
        let paths = extract_pdf_pages(score_path, pages, &page_dir)?;
        Ok((paths, true)) // true = clean up temp dir
    } else {
        Err("Either pages_dir or score_path must be set".to_string())
    }
}

/// Generate annotations for a single number by sending score pages to Claude.
pub async fn generate_number_annotation(
    config: &GenerateConfig,
    base: &BaseLibretto,
    number_id: &str,
    pages: &[u32],
) -> Result<NumberAnnotation, String> {
    tracing::info!(number_id = %number_id, pages = ?pages, "Generating annotation");

    // 1. Get page images (pre-ingested or extracted)
    let (image_paths, cleanup) = resolve_page_images(config, pages, number_id)?;
    tracing::info!(images = image_paths.len(), "Score pages ready");

    // 2. Build prompt and call Claude
    let prompt = build_prompt(base, number_id)?;

    tracing::info!(model = %config.model, images = image_paths.len(), "Calling Claude API");
    let text = call_claude(&config.api_key, &config.model, &image_paths, &prompt).await?;

    let json_text = extract_json_from_response(&text)
        .ok_or_else(|| format!("No JSON object found in LLM response:\n{text}"))?;

    let llm_resp: LlmAnnotationResponse = serde_json::from_str(&json_text)
        .map_err(|e| format!("Failed to parse LLM JSON: {e}\nText: {json_text}"))?;

    tracing::info!(
        tempo = ?llm_resp.tempo_marking,
        groups = llm_resp.groups.len(),
        "Received annotation from LLM"
    );

    // 6. Convert to NumberAnnotation
    let annotation = NumberAnnotation {
        number_id: number_id.to_string(),
        tempo_marking: llm_resp.tempo_marking,
        number_type: None,
        groups: llm_resp.groups.into_iter()
            .map(|g| GroupAnnotation {
                group: g.group,
                segment_ids: g.segment_ids,
            })
            .collect(),
    };

    // 7. Clean up temp images if we extracted them
    if cleanup {
        let page_dir = config.temp_dir.join(number_id);
        let _ = std::fs::remove_dir_all(&page_dir);
    }

    Ok(annotation)
}

/// Call Claude API with images and a text prompt, return the text response.
/// Retries with exponential backoff on rate limit (429) errors.
async fn call_claude(
    api_key: &str,
    model: &str,
    image_paths: &[PathBuf],
    prompt: &str,
) -> Result<String, String> {
    let mut content_parts: Vec<serde_json::Value> = Vec::new();
    for img_path in image_paths {
        let img_data = std::fs::read(img_path)
            .map_err(|e| format!("Failed to read image {}: {e}", img_path.display()))?;
        let b64 = base64::engine::general_purpose::STANDARD.encode(&img_data);
        content_parts.push(serde_json::json!({
            "type": "image",
            "source": {
                "type": "base64",
                "media_type": "image/png",
                "data": b64,
            }
        }));
    }
    content_parts.push(serde_json::json!({
        "type": "text",
        "text": prompt,
    }));

    let request_body = serde_json::json!({
        "model": model,
        "max_tokens": 4096,
        "messages": [{
            "role": "user",
            "content": content_parts,
        }]
    });

    let client = reqwest::Client::new();
    let max_retries = 5;
    let mut backoff_secs = 30u64;

    for attempt in 0..=max_retries {
        let response = client
            .post("https://api.anthropic.com/v1/messages")
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request_body)
            .send()
            .await
            .map_err(|e| format!("API request failed: {e}"))?;

        let status = response.status();
        let body = response.text().await
            .map_err(|e| format!("Failed to read response: {e}"))?;

        if status.as_u16() == 429 && attempt < max_retries {
            tracing::warn!(
                attempt = attempt + 1,
                backoff_secs = backoff_secs,
                "Rate limited, waiting before retry"
            );
            tokio::time::sleep(std::time::Duration::from_secs(backoff_secs)).await;
            backoff_secs = (backoff_secs * 2).min(120);
            continue;
        }

        if !status.is_success() {
            return Err(format!("Claude API error ({}): {}", status, body));
        }

        let claude_resp: ClaudeResponse = serde_json::from_str(&body)
            .map_err(|e| format!("Failed to parse Claude response: {e}"))?;

        return claude_resp.content.iter()
            .find_map(|b| b.text.clone())
            .ok_or_else(|| "No text in Claude response".to_string());
    }

    Err("Max retries exceeded".to_string())
}

/// Phase 1: Describe score pages using LLM.
///
/// Sends batches of page images to Claude and asks it to describe what it
/// sees — content type, title, characters, first/last words. No reference
/// to the base libretto is given; Claude just reports what's on the page.
pub async fn describe_pages(
    pages_dir: &Path,
    api_key: &str,
    model: &str,
    batch_size: usize,
) -> Result<super::PageDescriptions, String> {
    let mut page_files = collect_page_files(pages_dir)?;
    page_files.sort_by_key(|(n, _)| *n);

    let total_pages = page_files.len() as u32;
    tracing::info!(total_pages = total_pages, batch_size = batch_size, "Describing score pages");

    let mut all_descs: Vec<super::PageDescription> = Vec::new();

    for chunk in page_files.chunks(batch_size) {
        let first_page = chunk.first().map(|(n, _)| *n).unwrap_or(0);
        let last_page = chunk.last().map(|(n, _)| *n).unwrap_or(0);
        tracing::info!(pages = %format!("{first_page}-{last_page}"), "Processing batch");

        let image_paths: Vec<PathBuf> = chunk.iter().map(|(_, p)| p.clone()).collect();

        let prompt = format!(
r#"You are analyzing pages from a vocal score (opera). The pages shown are pages {first_page} through {last_page}.

For EACH page, describe what you see. Report ONLY what is visible on the page — do not guess or infer.

For each page, provide:
- "page": the page number
- "content_type": one of "overture", "number", "recitative", "front_matter", "other"
  - "overture" = orchestral music, no vocal staves
  - "number" = a musical number (aria, duet, trio, quartet, ensemble, finale, chorus, etc.)
  - "recitative" = recitative passage (typically sparse accompaniment, speech-like vocal line)
  - "front_matter" = title page, table of contents, index, etc.
  - "other" = blank or unrecognizable
- "title": any title or heading printed on the page (e.g., "N° 15 Terzetto", "RECITATIVO", "Aria"). Set to null if no title/heading is visible.
- "characters": array of character names visible on the vocal staves (e.g., ["SUSANNA", "FIGARO"]). Empty array if none visible.
- "first_words": the first few words of sung/spoken text on the page (3-8 words). null if no text visible.
- "last_words": the last few words of sung/spoken text on the page (3-8 words). null if no text visible.
- "continuation": true if this page continues a piece from the previous page (no new title or heading appears at the top), false if a new piece starts on this page.

Respond with ONLY a JSON array (no markdown, no explanation):
[
  {{"page": {first_page}, "content_type": "number", "title": "N° 1 Duettino", "characters": ["SUSANNA", "FIGARO"], "first_words": "Cinque dieci venti", "last_words": "il letto", "continuation": false}},
  ...
]
"#,
            first_page = first_page,
            last_page = last_page,
        );

        let text = call_claude(api_key, model, &image_paths, &prompt).await?;

        let json_text = extract_json_from_response(&text)
            .ok_or_else(|| format!("No JSON found in describe response for pages {first_page}-{last_page}:\n{text}"))?;

        let batch: Vec<super::PageDescription> = serde_json::from_str(&json_text)
            .map_err(|e| {
                format!("Failed to parse page descriptions JSON: {e}\nText: {}",
                    &json_text[..json_text.len().min(500)])
            })?;

        tracing::info!(entries = batch.len(), "Received page descriptions");
        all_descs.extend(batch);
    }

    all_descs.sort_by_key(|d| d.page);

    Ok(super::PageDescriptions {
        total_pages,
        pages: all_descs,
    })
}

/// Phase 2: Match page descriptions to base libretto numbers.
///
/// Uses content type, character names, text matching, and contiguity
/// to assign pages to musical number IDs. Character names from the
/// score are often abbreviated (e.g., "S" for SUSANNA, "F" for FIGARO),
/// so we use prefix-based matching with known abbreviation mappings.
pub fn match_pages_to_numbers(
    descs: &super::PageDescriptions,
    base: &BaseLibretto,
) -> super::ScoreMap {
    use super::PageContentType;

    // Known abbreviation → full character name mappings for Figaro
    // The matcher also does prefix matching, but these handle ambiguous cases.
    let abbreviation_map: Vec<(&str, &str)> = vec![
        ("SUS", "SUSANNA"), ("S", "SUSANNA"),
        ("FIG", "FIGARO"), ("F", "FIGARO"),
        ("C", "CONTE"), ("CONT", "CONTE"), ("C°", "CONTE"), ("C.", "CONTE"),
        ("Cc", "CONTESSA"), ("Cb", "CONTESSA"), ("Cb.", "CONTESSA"),
        ("BAR", "BARTOLO"), ("BAS", "BASILIO"),
        ("MAR", "MARCELLINA"), ("M", "MARCELLINA"),
        ("ANT", "ANTONIO"), ("A", "ANTONIO"),
        ("CHER", "CHERUBINO"), ("CH", "CHERUBINO"), ("Ch", "CHERUBINO"), ("Ch.", "CHERUBINO"),
        ("D.C", "CURZIO"), ("D.CUR", "CURZIO"), ("CUR", "CURZIO"),
        ("B", "BARTOLO"),  // Single "B" is ambiguous but usually Bartolo
    ];

    // Expand a score abbreviation to a full character name
    let expand_char = |abbr: &str| -> Option<String> {
        // Strip trailing dots/periods (e.g., "CONT." → "CONT", "FIG." → "FIG")
        let cleaned = abbr.trim().trim_end_matches('.');
        let upper = cleaned.to_uppercase();
        if upper.is_empty() { return None; }
        // Direct match first
        let known = ["SUSANNA", "FIGARO", "CONTE", "CONTESSA", "BARTOLO",
                      "BASILIO", "MARCELLINA", "CHERUBINO", "ANTONIO",
                      "BARBARINA", "CURZIO", "CORO"];
        for &k in &known {
            if upper == k { return Some(k.to_string()); }
        }
        // Abbreviation map (case-insensitive check, also try without dots)
        for &(abbr_key, full) in &abbreviation_map {
            if upper == abbr_key.to_uppercase() {
                return Some(full.to_string());
            }
        }
        // Prefix match against known names (require at least 2 chars)
        for &k in &known {
            if k.starts_with(&upper) && upper.len() >= 2 {
                return Some(k.to_string());
            }
        }
        None
    };

    // Expand all character names from a page description, handling compound
    // entries like "S B" or "ANT BAR E FIG"
    let expand_chars = |raw_chars: &[String]| -> Vec<String> {
        let mut result = std::collections::HashSet::new();
        for raw in raw_chars {
            // Split compound entries on spaces, commas, "E" (Italian "and")
            let parts: Vec<&str> = raw.split(|c: char| c == ' ' || c == ',')
                .map(|s| s.trim())
                .filter(|s| !s.is_empty() && *s != "E" && *s != "e")
                .collect();
            for part in parts {
                if let Some(full) = expand_char(part) {
                    result.insert(full);
                }
            }
            // Also try the whole string as one name
            if let Some(full) = expand_char(raw) {
                result.insert(full);
            }
        }
        result.into_iter().collect()
    };

    // Build a lookup of number characteristics from the base libretto
    struct NumberInfo {
        id: String,
        _label: String,
        _act: String,
        number_type: crate::base_libretto::NumberType,
        characters: Vec<String>,
        first_text: String,
        all_text: String,
    }

    let number_infos: Vec<NumberInfo> = base.numbers.iter().map(|n| {
        let mut seen = std::collections::HashSet::new();
        let characters: Vec<String> = n.segments.iter()
            .filter(|s| s.segment_type == crate::base_libretto::SegmentType::Sung)
            .filter_map(|s| s.character.as_deref())
            // Split compound characters like "SUSANNA, FIGARO" into individual names
            .flat_map(|c| c.split(',').map(|s| s.trim().to_uppercase()))
            .filter(|c| !c.is_empty() && seen.insert(c.clone()))
            .collect();

        // First line of text from first sung segment (for word-level matching)
        let first_text = n.segments.iter()
            .filter(|s| s.segment_type == crate::base_libretto::SegmentType::Sung)
            .find_map(|s| s.text.as_deref())
            .unwrap_or("")
            .to_lowercase();

        // All sung text joined (for substring matching of quoted titles)
        let all_text: String = n.segments.iter()
            .filter(|s| s.segment_type == crate::base_libretto::SegmentType::Sung)
            .filter_map(|s| s.text.as_deref())
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase();

        NumberInfo {
            id: n.id.clone(),
            _label: n.label.clone(),
            _act: n.act.clone(),
            number_type: n.number_type.clone(),
            characters,
            first_text,
            all_text,
        }
    }).collect();

    // Extract quoted text from a title string (e.g., "N° 24. ARIA \"Voi che sapete\"" → "voi che sapete")
    let extract_title_text = |title: &str| -> Option<String> {
        // Try various quote patterns: "...", «...», '...'
        let start = title.find('"').or_else(|| title.find('«')).or_else(|| title.find('\u{201C}'));
        if let Some(s) = start {
            let after = &title[s + 1..];
            let end = after.find('"').or_else(|| after.find('»')).or_else(|| after.find('\u{201D}'));
            if let Some(e) = end {
                let text = after[..e].trim();
                if !text.is_empty() {
                    return Some(text.to_lowercase());
                }
            }
            // No closing quote — take the rest
            let text = after.trim().trim_end_matches(|c: char| c == '"' || c == ',' || c == '.');
            if !text.is_empty() {
                return Some(text.to_lowercase());
            }
        }
        None
    };

    // Score each (page_description, number) pair
    let score_match = |desc_chars: &[String], desc: &super::PageDescription, info: &NumberInfo| -> i32 {
        let mut score = 0i32;

        // Character overlap using expanded names
        let char_matches = desc_chars.iter()
            .filter(|dc| info.characters.iter().any(|ic| ic == *dc))
            .count();
        score += (char_matches as i32) * 10;

        // Penalize if page has characters NOT in the number
        let char_misses = desc_chars.iter()
            .filter(|dc| !info.characters.iter().any(|ic| ic == *dc))
            .count();
        score -= (char_misses as i32) * 5;

        // Title quoted text matching via SUBSTRING (strongest text signal)
        // Titles like 'N° 24. ARIA "Voi che sapete"' contain identifying lyrics.
        // We normalize both sides and check if the quoted text appears as a
        // substring in the number's full text. This avoids false matches on
        // common short words like "che", "non", etc.
        if let Some(ref title) = desc.title {
            if let Some(quoted) = extract_title_text(title) {
                // Normalize: strip accents + punctuation, collapse whitespace
                let strip_accents = |s: &str| -> String {
                    s.chars().map(|c| match c {
                        'à' | 'á' | 'â' | 'ã' | 'ä' => 'a',
                        'è' | 'é' | 'ê' | 'ë' => 'e',
                        'ì' | 'í' | 'î' | 'ï' => 'i',
                        'ò' | 'ó' | 'ô' | 'õ' | 'ö' => 'o',
                        'ù' | 'ú' | 'û' | 'ü' => 'u',
                        _ => c,
                    }).collect()
                };
                let norm_quoted: String = strip_accents(&quoted).chars()
                    .filter(|c| c.is_alphanumeric() || c.is_whitespace())
                    .collect::<String>()
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");
                let norm_all: String = strip_accents(&info.all_text).chars()
                    .filter(|c| c.is_alphanumeric() || c.is_whitespace())
                    .collect::<String>()
                    .split_whitespace()
                    .collect::<Vec<_>>()
                    .join(" ");

                if norm_quoted.len() >= 5 && norm_all.contains(&norm_quoted) {
                    // Very strong match — the title lyrics appear in this number
                    score += 50;
                } else if norm_quoted.len() >= 5 {
                    // Try matching first 3 words as a prefix
                    let prefix: String = norm_quoted.split_whitespace()
                        .take(3)
                        .collect::<Vec<_>>()
                        .join(" ");
                    if prefix.len() >= 5 && norm_all.contains(&prefix) {
                        score += 30;
                    }
                }
            }
        }

        // First_words text matching against first_text
        if let Some(ref first_words) = desc.first_words {
            let words_lower = first_words.to_lowercase();
            let desc_words: Vec<&str> = words_lower.split_whitespace().collect();
            let info_words: Vec<&str> = info.first_text.split_whitespace().collect();
            let word_matches = desc_words.iter()
                .filter(|dw| info_words.iter().any(|iw| iw == *dw))
                .count();
            score += (word_matches as i32) * 3;

            // Also try space-stripped substring match (handles OCR splits like "A pri te" → "aprite")
            let stripped_desc: String = words_lower.chars().filter(|c| c.is_alphanumeric()).collect();
            let stripped_info: String = info.first_text.chars().filter(|c| c.is_alphanumeric()).collect();
            let prefix_end = stripped_desc.char_indices().nth(12).map(|(i, _)| i).unwrap_or(stripped_desc.len());
            if stripped_desc.len() >= 6 && stripped_info.starts_with(&stripped_desc[..prefix_end]) {
                score += 30;
            }
        }

        // Title type matching
        if let Some(ref title) = desc.title {
            let title_lower = title.to_lowercase();
            let type_pairs: &[(&str, &[crate::base_libretto::NumberType])] = &[
                ("duettino", &[crate::base_libretto::NumberType::Duettino]),
                ("duetto", &[crate::base_libretto::NumberType::Duet, crate::base_libretto::NumberType::Duettino]),
                ("aria", &[crate::base_libretto::NumberType::Aria, crate::base_libretto::NumberType::RecitativeAccompagnato]),
                ("cavatina", &[crate::base_libretto::NumberType::Cavatina]),
                ("terzetto", &[crate::base_libretto::NumberType::Terzetto]),
                ("coro", &[crate::base_libretto::NumberType::Chorus]),
                ("canzone", &[crate::base_libretto::NumberType::Canzone]),
                ("sestetto", &[crate::base_libretto::NumberType::Sextet]),
                ("finale", &[crate::base_libretto::NumberType::Finale]),
                ("marcia", &[crate::base_libretto::NumberType::March]),
            ];
            for (italian, types) in type_pairs {
                if title_lower.contains(italian) && types.contains(&info.number_type) {
                    score += 8;
                }
            }
        }

        score
    };

    // Deduplicate pages (Claude sometimes returns multiple entries for the same page)
    let mut seen_pages = std::collections::HashSet::new();
    let unique_descs: Vec<&super::PageDescription> = descs.pages.iter()
        .filter(|d| seen_pages.insert(d.page))
        .collect();

    // Walk through pages in order, assigning to numbers
    let mut page_assignments: Vec<(u32, Option<String>)> = Vec::new();
    let mut current_number: Option<String> = None;
    // Track which numbers already have a non-continuation start page
    let mut started_numbers: std::collections::HashSet<String> = std::collections::HashSet::new();

    for desc in &unique_descs {
        match desc.content_type {
            PageContentType::FrontMatter | PageContentType::Other => {
                page_assignments.push((desc.page, None));
                current_number = None;
            }
            PageContentType::Overture => {
                let overture_id = number_infos.iter()
                    .find(|n| n.number_type == crate::base_libretto::NumberType::Overture)
                    .map(|n| n.id.clone());
                page_assignments.push((desc.page, overture_id.clone()));
                current_number = overture_id;
            }
            PageContentType::Recitative => {
                // Check if current number is a finale
                let in_finale = current_number.as_ref().map_or(false, |id| {
                    number_infos.iter().find(|n| n.id == *id)
                        .map_or(false, |n| n.number_type == crate::base_libretto::NumberType::Finale)
                });

                // Check if title is a scene heading (not a new numbered piece)
                let is_scene_heading = desc.title.as_ref().map_or(false, |t| {
                    let tl = t.to_lowercase();
                    tl.starts_with("scena") || tl.starts_with("atto")
                });

                if desc.continuation && current_number.is_some() {
                    // Continuation recitative inside a number (e.g., recit between
                    // scenes in a finale) — keep in current number
                    page_assignments.push((desc.page, current_number.clone()));
                } else if in_finale && (is_scene_heading || desc.title.is_none()) {
                    // Recitative scene change inside a finale — keep in finale
                    page_assignments.push((desc.page, current_number.clone()));
                } else {
                    // Check if this recitative actually starts a numbered piece
                    // (e.g., "Recitativo ed Aria")
                    let is_number_start = desc.title.as_ref().map_or(false, |t| {
                        let tl = t.to_lowercase();
                        tl.contains("aria") || tl.contains("duett") || tl.contains("terzett")
                            || tl.contains("cavatin") || tl.contains("canzon")
                            || tl.contains("sestett") || tl.contains("finale")
                    });
                    if is_number_start {
                        let desc_chars = expand_chars(&desc.characters);
                        let mut best_id: Option<String> = None;
                        let mut best_score = 0i32;
                        for info in &number_infos {
                            let mut s = score_match(&desc_chars, desc, info);
                            // Penalize numbers that already have a start page
                            if started_numbers.contains(&info.id) {
                                s -= 20;
                            }
                            if s > best_score {
                                best_score = s;
                                best_id = Some(info.id.clone());
                            }
                        }
                        if best_score >= 5 {
                            tracing::info!(
                                page = desc.page,
                                number_id = ?best_id,
                                score = best_score,
                                title = ?desc.title,
                                "Recitative page matched as number start"
                            );
                        }
                        if let Some(ref id) = best_id {
                            started_numbers.insert(id.clone());
                        }
                        page_assignments.push((desc.page, best_id.clone()));
                        current_number = best_id;
                    } else {
                        page_assignments.push((desc.page, None));
                        current_number = None;
                    }
                }
            }
            PageContentType::Number => {
                // Check if current number is a finale
                let in_finale = current_number.as_ref().map_or(false, |id| {
                    number_infos.iter().find(|n| n.id == *id)
                        .map_or(false, |n| n.number_type == crate::base_libretto::NumberType::Finale)
                });

                // Scene/tempo headings inside a finale should stay in the finale
                // even when Claude marks them as continuation=false
                let is_scene_heading = desc.title.as_ref().map_or(false, |t| {
                    let tl = t.to_lowercase();
                    (tl.starts_with("scena") || tl.starts_with("atto")
                     || tl == "allegro" || tl == "andante" || tl == "presto"
                     || tl.starts_with("più "))
                    && !tl.contains("n°") && !tl.contains("n.")
                });

                let stay_in_current = (desc.continuation && current_number.is_some())
                    || (in_finale && (desc.continuation || is_scene_heading || desc.title.is_none()));

                if stay_in_current {
                    page_assignments.push((desc.page, current_number.clone()));
                } else {
                    // New number starts — find best match
                    let desc_chars = expand_chars(&desc.characters);
                    let mut best_id: Option<String> = None;
                    let mut best_score = 0i32;

                    for info in &number_infos {
                        let mut s = score_match(&desc_chars, desc, info);
                        // Penalize numbers that already have a start page
                        if started_numbers.contains(&info.id) {
                            s -= 20;
                        }
                        if s > best_score {
                            best_score = s;
                            best_id = Some(info.id.clone());
                        }
                    }

                    if best_score < 5 {
                        tracing::warn!(
                            page = desc.page,
                            title = ?desc.title,
                            characters = ?desc.characters,
                            expanded = ?desc_chars,
                            first_words = ?desc.first_words,
                            best_score = best_score,
                            best_match = ?best_id,
                            "Low confidence match"
                        );
                    } else {
                        tracing::info!(
                            page = desc.page,
                            number_id = ?best_id,
                            score = best_score,
                            title = ?desc.title,
                            expanded = ?desc_chars,
                            "Matched page to number"
                        );
                    }

                    if let Some(ref id) = best_id {
                        started_numbers.insert(id.clone());
                    }
                    page_assignments.push((desc.page, best_id.clone()));
                    current_number = best_id;
                }
            }
        }
    }

    // Track which pages were explicit non-continuation starts for each number
    // (these are high-confidence anchor points)
    let mut number_start_pages: std::collections::HashMap<String, u32> =
        std::collections::HashMap::new();
    for desc in &unique_descs {
        if !desc.continuation {
            if let Some((_, Some(ref id))) = page_assignments.iter().find(|(p, _)| *p == desc.page) {
                number_start_pages.entry(id.clone()).or_insert(desc.page);
            }
        }
    }

    // Group assigned pages by number_id
    let mut number_pages: std::collections::BTreeMap<String, Vec<u32>> =
        std::collections::BTreeMap::new();

    for (page, number_id) in &page_assignments {
        if let Some(id) = number_id {
            number_pages.entry(id.clone()).or_default().push(*page);
        }
    }

    // Build entries preserving the order numbers appear in the base libretto.
    // For each number, keep only one contiguous block of pages.
    // Prefer the block containing the first non-continuation start page.
    let mut entries: Vec<super::ScoreMapEntry> = Vec::new();
    for info in &number_infos {
        if let Some(mut pages) = number_pages.remove(&info.id) {
            pages.sort();
            pages.dedup();

            // Split into contiguous runs (allow gap of 1 for recitative pages)
            let mut runs: Vec<Vec<u32>> = Vec::new();
            let mut current_run: Vec<u32> = vec![pages[0]];
            for i in 1..pages.len() {
                if pages[i] <= pages[i - 1] + 2 {
                    current_run.push(pages[i]);
                } else {
                    runs.push(current_run);
                    current_run = vec![pages[i]];
                }
            }
            runs.push(current_run);

            // Pick the best run: prefer the one containing the start page,
            // then extend forward by merging subsequent runs with small gaps
            let start_page = number_start_pages.get(&info.id);
            let start_run_idx = if let Some(&sp) = start_page {
                runs.iter().position(|r| r.contains(&sp)).unwrap_or(0)
            } else {
                // No start page — pick the largest run's index
                runs.iter().enumerate().max_by_key(|(_, r)| r.len()).map(|(i, _)| i).unwrap_or(0)
            };

            // Merge the start run forward with nearby runs (gap ≤ 5 pages)
            let mut merged: Vec<u32> = runs[start_run_idx].clone();
            for run in runs.iter().skip(start_run_idx + 1) {
                let last = *merged.last().unwrap();
                let first = run[0];
                if first <= last + 6 {
                    merged.extend(run);
                } else {
                    break;
                }
            }
            let best_run = &merged;

            if best_run.len() < pages.len() {
                tracing::info!(
                    number_id = %info.id,
                    kept = ?best_run,
                    total_assigned = pages.len(),
                    start_page = ?start_page,
                    "Kept contiguous block around start page"
                );
            }

            entries.push(super::ScoreMapEntry {
                number_id: info.id.clone(),
                score_label: None,
                pages: best_run.clone(),
            });
        }
    }

    let mapped_count = entries.len();
    let unmatched_pages = page_assignments.iter().filter(|(_, id)| id.is_none()).count();
    let missing: Vec<&str> = number_infos.iter()
        .filter(|n| !entries.iter().any(|e| e.number_id == n.id))
        .map(|n| n.id.as_str())
        .collect();

    tracing::info!(
        mapped = mapped_count,
        unmatched_pages = unmatched_pages,
        missing_numbers = ?missing,
        "Page matching complete"
    );

    super::ScoreMap {
        total_pages: descs.total_pages,
        numbers: entries,
    }
}

/// Find all page image files in a directory (p001.png, p002.png, etc.).
fn collect_page_files(pages_dir: &Path) -> Result<Vec<(u32, PathBuf)>, String> {
    let mut page_files: Vec<(u32, PathBuf)> = Vec::new();
    for entry in std::fs::read_dir(pages_dir)
        .map_err(|e| format!("Failed to read pages dir: {e}"))?
    {
        let entry = entry.map_err(|e| format!("Dir entry error: {e}"))?;
        let name = entry.file_name().to_string_lossy().to_string();
        if let Some(num_str) = name.strip_prefix('p').and_then(|s| s.strip_suffix(".png")) {
            if let Ok(num) = num_str.parse::<u32>() {
                page_files.push((num, entry.path()));
            }
        }
    }
    Ok(page_files)
}
