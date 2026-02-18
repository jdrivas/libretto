// Segment grouping annotation using LLM + score page images.
//
// Reads a merged_score_libretto.json, sends each number's segments and
// score page images to Claude, and annotates segments with group keys
// indicating simultaneous singing.

use std::collections::BTreeSet;
use std::path::PathBuf;

use anyhow::{Context, Result};
use base64::Engine;
use serde::{Deserialize, Serialize};

const CLAUDE_API_URL: &str = "https://api.anthropic.com/v1/messages";

/// Configuration for the grouping annotation pass.
#[derive(Debug, Clone)]
pub struct GroupingConfig {
    /// Path to merged_score_libretto.json.
    pub input_path: PathBuf,
    /// Directory containing score page images (p001.png, p002.png, ...).
    pub pages_dir: PathBuf,
    /// Output path for the annotated JSON.
    pub output_path: PathBuf,
    /// Anthropic API key.
    pub api_key: String,
    /// Claude model to use.
    pub model: String,
}

/// LLM response for a single number's grouping.
#[derive(Debug, Deserialize)]
struct LlmGroupingResponse {
    #[serde(default)]
    groups: Vec<LlmGroup>,
}

#[derive(Debug, Deserialize)]
struct LlmGroup {
    group: String,
    segment_ids: Vec<String>,
}

/// Summary of grouping results for reporting.
#[derive(Debug, Serialize)]
pub struct GroupingSummary {
    pub total_numbers: usize,
    pub numbers_with_segments: usize,
    pub numbers_processed: usize,
    pub numbers_with_groups: usize,
    pub total_groups: usize,
    pub total_grouped_segments: usize,
    pub errors: Vec<String>,
}

/// Run grouping annotation on all numbers in a merged_score_libretto.json.
pub async fn annotate_groups(config: &GroupingConfig) -> Result<GroupingSummary> {
    let input_text = std::fs::read_to_string(&config.input_path)
        .context("Failed to read input JSON")?;
    let mut doc: serde_json::Value = serde_json::from_str(&input_text)
        .context("Failed to parse input JSON")?;

    let numbers = doc.get_mut("numbers")
        .and_then(|v| v.as_array_mut())
        .ok_or_else(|| anyhow::anyhow!("No 'numbers' array in input JSON"))?;

    let total_numbers = numbers.len();
    let mut numbers_with_segments = 0;
    let mut numbers_processed = 0;
    let mut numbers_with_groups = 0;
    let mut total_groups = 0;
    let mut total_grouped_segments = 0;
    let mut errors = Vec::new();

    // Collect work items: (index, number_id, title, segments_json, page_numbers)
    let mut work: Vec<(usize, u64, String, String, Vec<u32>)> = Vec::new();

    for (i, number) in numbers.iter().enumerate() {
        let segments = number.get("segments").and_then(|v| v.as_array());
        let seg_count = segments.map_or(0, |s| s.len());
        if seg_count == 0 {
            continue;
        }
        numbers_with_segments += 1;

        let number_id = number.get("id").and_then(|v| v.as_u64()).unwrap_or(0);
        let title = number.get("title").and_then(|v| v.as_str()).unwrap_or("").to_string();

        // Build segment summary for the prompt
        let segments_json = build_segment_summary(segments.unwrap());

        // Collect unique page numbers
        let pages = number.get("pages").and_then(|v| v.as_array());
        let page_nums: Vec<u32> = if let Some(pages) = pages {
            let mut seen = BTreeSet::new();
            for p in pages {
                if let Some(pg) = p.get("page").and_then(|v| v.as_u64()) {
                    seen.insert(pg as u32);
                }
            }
            seen.into_iter().collect()
        } else {
            vec![]
        };

        work.push((i, number_id, title, segments_json, page_nums));
    }

    tracing::info!(
        total = total_numbers,
        with_segments = numbers_with_segments,
        "Starting grouping annotation"
    );

    let client = reqwest::Client::new();

    for (idx, number_id, title, segments_json, page_nums) in &work {
        let form = numbers[*idx].get("form").and_then(|v| v.as_str()).unwrap_or("?");

        tracing::info!(
            number_id = number_id,
            form = form,
            title = %title,
            pages = page_nums.len(),
            "Processing number"
        );

        // Load page images
        let image_paths: Vec<PathBuf> = page_nums.iter()
            .map(|&p| config.pages_dir.join(format!("p{p:03}.png")))
            .collect();

        // Verify images exist
        let mut missing = false;
        for p in &image_paths {
            if !p.exists() {
                let msg = format!("Number {}: missing page image {}", number_id, p.display());
                tracing::warn!("{}", msg);
                errors.push(msg);
                missing = true;
            }
        }
        if missing {
            continue;
        }

        // Build prompt
        let prompt = build_grouping_prompt(*number_id, title, form, segments_json);

        // Call Claude
        match call_claude_with_images(&client, &config.api_key, &config.model, &image_paths, &prompt).await {
            Ok(response_text) => {
                match parse_grouping_response(&response_text) {
                    Ok(groups) => {
                        numbers_processed += 1;
                        if !groups.is_empty() {
                            numbers_with_groups += 1;
                            total_groups += groups.len();

                            // Apply groups to segments
                            let grouped = apply_groups_to_number(
                                &mut numbers[*idx], &groups,
                            );
                            total_grouped_segments += grouped;

                            tracing::info!(
                                number_id = number_id,
                                groups = groups.len(),
                                segments_grouped = grouped,
                                "Applied grouping"
                            );
                        } else {
                            numbers_processed += 1;
                            tracing::info!(number_id = number_id, "No groups found (sequential)");
                        }
                    }
                    Err(e) => {
                        let msg = format!("Number {}: failed to parse response: {}", number_id, e);
                        tracing::warn!("{}", msg);
                        errors.push(msg);
                    }
                }
            }
            Err(e) => {
                let msg = format!("Number {}: Claude API error: {}", number_id, e);
                tracing::warn!("{}", msg);
                errors.push(msg);
            }
        }
    }

    // Write output
    let output_json = serde_json::to_string_pretty(&doc)?;
    if let Some(parent) = config.output_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&config.output_path, &output_json)?;
    tracing::info!(path = %config.output_path.display(), "Wrote annotated JSON");

    Ok(GroupingSummary {
        total_numbers,
        numbers_with_segments,
        numbers_processed,
        numbers_with_groups,
        total_groups,
        total_grouped_segments,
        errors,
    })
}

/// Build a compact segment summary for the prompt.
fn build_segment_summary(segments: &[serde_json::Value]) -> String {
    let mut lines = Vec::new();
    for seg in segments {
        let id = seg.get("id").and_then(|v| v.as_str()).unwrap_or("?");
        let character = seg.get("character").and_then(|v| v.as_str()).unwrap_or("?");
        let text = seg.get("text").and_then(|v| v.as_str()).unwrap_or("");
        // Truncate long text for the prompt
        let text_short = if text.len() > 80 {
            format!("{}...", &text[..text.char_indices().nth(77).map_or(text.len(), |(i,_)| i)])
        } else {
            text.to_string()
        };
        // Replace newlines with spaces for compact display
        let text_clean = text_short.replace('\n', " ");
        lines.push(format!("  {} | {} | {}", id, character, text_clean));
    }
    lines.join("\n")
}

/// Build the Claude prompt for grouping a single number.
fn build_grouping_prompt(number_id: u64, title: &str, form: &str, segments: &str) -> String {
    format!(
r#"You are analyzing vocal score pages for musical number #{number_id}: "{title}" (form: {form}).

Here are the libretto segments for this number, in order:

  SEGMENT_ID | CHARACTER | TEXT
{segments}

The score pages for this number are attached as images.

TASK: Identify which segments are sung SIMULTANEOUSLY (multiple voices singing at the same time). Assign a group label to segments that overlap in time.

Rules:
- Only group segments that genuinely overlap in the score (you can see multiple vocal staves with different lyrics active at the same beat position)
- Segments where characters alternate (one finishes, then the next begins) are NOT grouped
- In recitatives, characters almost never sing simultaneously — typically return empty groups
- Use sequential group labels within this number: "g1", "g2", "g3", etc.
- A group must contain at least 2 segment IDs
- The same segment can only belong to one group
- If no segments are simultaneous, return an empty groups array

Respond with ONLY a JSON object (no markdown fences, no explanation):
{{
  "groups": [
    {{
      "group": "g1",
      "segment_ids": ["segment-id-1", "segment-id-2"]
    }}
  ]
}}"#,
        number_id = number_id,
        title = title,
        form = form,
        segments = segments,
    )
}

/// Parse the LLM response into group assignments.
fn parse_grouping_response(text: &str) -> Result<Vec<LlmGroup>> {
    let json_text = extract_json_from_response(text)
        .ok_or_else(|| anyhow::anyhow!("No JSON found in response"))?;

    let resp: LlmGroupingResponse = serde_json::from_str(&json_text)
        .context(format!("Failed to parse grouping JSON: {}", &json_text[..json_text.len().min(300)]))?;

    Ok(resp.groups)
}

/// Apply group assignments to a number's segments in the JSON value.
/// Returns the count of segments that received a group key.
fn apply_groups_to_number(number: &mut serde_json::Value, groups: &[LlmGroup]) -> usize {
    let segments = match number.get_mut("segments").and_then(|v| v.as_array_mut()) {
        Some(s) => s,
        None => return 0,
    };

    // Build a map: segment_id -> group label
    let mut seg_to_group = std::collections::HashMap::new();
    for g in groups {
        for sid in &g.segment_ids {
            seg_to_group.insert(sid.as_str(), g.group.as_str());
        }
    }

    let mut count = 0;
    for seg in segments.iter_mut() {
        if let Some(id) = seg.get("id").and_then(|v| v.as_str()).map(|s| s.to_string()) {
            if let Some(&group) = seg_to_group.get(id.as_str()) {
                seg.as_object_mut().unwrap().insert(
                    "group".to_string(),
                    serde_json::Value::String(group.to_string()),
                );
                count += 1;
            }
        }
    }

    count
}

/// Extract a JSON object from a response that may contain surrounding text.
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

    // Try 2: find outermost { } by bracket counting
    let mut depth = 0i32;
    let mut start_idx = None;
    for (i, ch) in trimmed.char_indices() {
        if ch == '{' {
            if depth == 0 {
                start_idx = Some(i);
            }
            depth += 1;
        } else if ch == '}' {
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

    // Try 3: the whole thing is JSON
    if serde_json::from_str::<serde_json::Value>(trimmed).is_ok() {
        return Some(trimmed.to_string());
    }

    None
}

/// Call Claude API with images and a text prompt, return the text response.
/// Retries with exponential backoff on rate limit (429) errors.
async fn call_claude_with_images(
    client: &reqwest::Client,
    api_key: &str,
    model: &str,
    image_paths: &[PathBuf],
    prompt: &str,
) -> Result<String> {
    let mut content_parts: Vec<serde_json::Value> = Vec::new();

    for img_path in image_paths {
        let img_data = std::fs::read(img_path)
            .context(format!("Failed to read image {}", img_path.display()))?;
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

    let max_retries = 5;
    let mut backoff_secs = 30u64;

    for attempt in 0..=max_retries {
        let response = client
            .post(CLAUDE_API_URL)
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request_body)
            .send()
            .await
            .context("Claude API request failed")?;

        let status = response.status();
        let body = response.text().await
            .context("Failed to read Claude response")?;

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
            anyhow::bail!("Claude API error ({}): {}", status, body);
        }

        #[derive(Deserialize)]
        struct ClaudeResponse {
            content: Vec<ContentBlock>,
        }
        #[derive(Deserialize)]
        struct ContentBlock {
            text: Option<String>,
        }

        let claude_resp: ClaudeResponse = serde_json::from_str(&body)
            .context("Failed to parse Claude response")?;

        return claude_resp.content.iter()
            .find_map(|b| b.text.clone())
            .ok_or_else(|| anyhow::anyhow!("No text in Claude response"));
    }

    anyhow::bail!("Max retries exceeded for Claude API")
}
