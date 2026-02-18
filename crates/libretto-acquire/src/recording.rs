// Recording acquisition: LLM-assisted search + Discogs.
//
// Parses a free-text query describing a recording (conductor, year, label,
// orchestra, artists), uses Claude to identify the recording and find
// Wikipedia/web sources, then optionally fetches track listings from Discogs.

use anyhow::{Context, Result};
use scraper::{ElementRef, Html, Selector};
use serde::{Deserialize, Serialize};
use std::path::Path;

const DISCOGS_API_BASE: &str = "https://api.discogs.com";
const USER_AGENT: &str = "libretto/0.1 (opera libretto tool)";

// ---------------------------------------------------------------------------
// Query parsing
// ---------------------------------------------------------------------------

/// Structured hints parsed from a free-text recording query.
#[derive(Debug, Clone)]
pub struct RecordingQuery {
    pub conductor: Option<String>,
    pub year: Option<u16>,
    pub label: Option<String>,
    pub orchestra: Option<String>,
    pub artists: Vec<String>,
    pub raw_query: String,
}

/// Known classical music record labels (lowercase for matching).
const KNOWN_LABELS: &[&str] = &[
    "decca", "deutsche grammophon", "dg", "emi", "philips", "sony", "rca",
    "columbia", "harmonia mundi", "erato", "chandos", "naxos", "archiv",
    "hyperion", "bis", "supraphon", "orfeo", "warner", "virgin",
    "london", "angel", "cbs", "teldec", "telarc",
];

/// Parse a free-text recording query into structured hints.
///
/// Expects a comma-separated string like:
/// "solti, 1981, decca, london philharmonic orchestra and chorus, with Kiri Te Kanawa"
pub fn parse_query(query: &str) -> RecordingQuery {
    let mut conductor = None;
    let mut year = None;
    let mut label = None;
    let mut orchestra = None;
    let mut artists = Vec::new();

    let parts: Vec<&str> = query.split(',').map(|s| s.trim()).filter(|s| !s.is_empty()).collect();

    for (i, part) in parts.iter().enumerate() {
        let lower = part.to_lowercase();

        // Check for year (4-digit number)
        if let Some(y) = extract_year(part) {
            year = Some(y);
            continue;
        }

        // Check for known label
        if KNOWN_LABELS.iter().any(|l| lower == *l) {
            label = Some(part.to_string());
            continue;
        }

        // Check for "with ..." prefix (additional artists)
        if lower.starts_with("with ") {
            artists.push(part[5..].trim().to_string());
            continue;
        }

        // Check for orchestra/ensemble keywords
        if contains_ensemble_keyword(&lower) {
            orchestra = Some(part.to_string());
            continue;
        }

        // First unmatched token is likely the conductor
        if conductor.is_none() && i == 0 {
            conductor = Some(part.to_string());
        } else {
            artists.push(part.to_string());
        }
    }

    RecordingQuery {
        conductor,
        year,
        label,
        orchestra,
        artists,
        raw_query: query.to_string(),
    }
}

fn extract_year(s: &str) -> Option<u16> {
    let trimmed = s.trim();
    if trimmed.len() == 4 {
        if let Ok(y) = trimmed.parse::<u16>() {
            if (1900..=2100).contains(&y) {
                return Some(y);
            }
        }
    }
    None
}

fn contains_ensemble_keyword(s: &str) -> bool {
    let keywords = [
        "orchestra", "philharmonic", "chorus", "choir",
        "ensemble", "opera house", "staatsoper", "philharmoniker",
    ];
    keywords.iter().any(|k| s.contains(k))
}

/// Convert an opera slug like "mozart/le-nozze-di-figaro" to a search-friendly title.
fn opera_slug_to_title(opera: &str) -> String {
    let slug = opera.rsplit('/').next().unwrap_or(opera);

    // If already in a readable format (has spaces or underscores), just return
    if slug.contains(' ') {
        return slug.to_string();
    }

    let sep = if slug.contains('_') { '_' } else { '-' };
    slug.split(sep)
        .map(|word| {
            let mut chars = word.chars();
            match chars.next() {
                None => String::new(),
                Some(c) => {
                    let upper: String = c.to_uppercase().collect();
                    upper + chars.as_str()
                }
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// ---------------------------------------------------------------------------
// Discogs API types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct DiscogsSearchResponse {
    #[serde(default)]
    results: Vec<DiscogsSearchResult>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct DiscogsSearchResult {
    pub id: u64,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub year: Option<String>,
    #[serde(default)]
    pub label: Option<Vec<String>>,
    #[serde(default)]
    pub format: Option<Vec<String>>,
    #[serde(default)]
    pub country: Option<String>,
    #[serde(default)]
    pub catno: Option<String>,
    #[serde(default)]
    pub resource_url: Option<String>,
    #[serde(default)]
    pub master_id: Option<u64>,
    #[serde(default)]
    pub format_quantity: Option<u32>,
    #[serde(default)]
    pub barcode: Option<Vec<String>>,
    #[serde(default)]
    pub uri: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DiscogsRelease {
    #[allow(dead_code)]
    id: u64,
    title: String,
    #[serde(default)]
    artists: Vec<DiscogsArtist>,
    #[serde(default)]
    labels: Vec<DiscogsLabel>,
    #[serde(default)]
    year: Option<u16>,
    #[serde(default)]
    tracklist: Vec<DiscogsTrack>,
}

#[derive(Debug, Deserialize)]
struct DiscogsArtist {
    name: String,
    #[serde(default)]
    role: Option<String>,
    #[allow(dead_code)]
    #[serde(default)]
    anv: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DiscogsLabel {
    name: String,
    #[allow(dead_code)]
    #[serde(default)]
    catno: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DiscogsTrack {
    #[serde(default)]
    position: String,
    #[serde(default)]
    title: String,
    #[serde(default)]
    duration: String,
    #[serde(default)]
    type_: String,
}

// ---------------------------------------------------------------------------
// Discogs API calls
// ---------------------------------------------------------------------------

async fn search_discogs(
    query: &RecordingQuery,
    opera_title: &str,
    token: &str,
) -> Result<Vec<DiscogsSearchResult>> {
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()?;

    let mut params: Vec<(&str, String)> = vec![
        ("type", "release".to_string()),
        ("per_page", "10".to_string()),
    ];

    // Build the search query combining opera title and conductor
    let mut q_parts = vec![opera_title.to_string()];
    if let Some(ref conductor) = query.conductor {
        q_parts.push(conductor.clone());
    }
    params.push(("q", q_parts.join(" ")));

    if let Some(y) = query.year {
        params.push(("year", y.to_string()));
    }
    if let Some(ref label) = query.label {
        params.push(("label", label.clone()));
    }

    let url = format!("{DISCOGS_API_BASE}/database/search");
    tracing::info!(url = %url, "Searching Discogs");

    let response = client
        .get(&url)
        .header("Authorization", format!("Discogs token={token}"))
        .query(&params)
        .send()
        .await
        .context("Failed to search Discogs")?;

    let status = response.status();
    let body = response.text().await.context("Failed to read Discogs response")?;

    if !status.is_success() {
        anyhow::bail!("Discogs search failed ({}): {}", status, body);
    }

    let search_resp: DiscogsSearchResponse = serde_json::from_str(&body)
        .context("Failed to parse Discogs search response")?;

    tracing::info!(results = search_resp.results.len(), "Discogs search complete");

    Ok(search_resp.results)
}

async fn fetch_release(release_id: u64, token: &str) -> Result<DiscogsRelease> {
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()?;

    let url = format!("{DISCOGS_API_BASE}/releases/{release_id}");
    tracing::info!(url = %url, "Fetching release details");

    let response = client
        .get(&url)
        .header("Authorization", format!("Discogs token={token}"))
        .send()
        .await
        .context("Failed to fetch Discogs release")?;

    let status = response.status();
    let body = response.text().await.context("Failed to read release response")?;

    if !status.is_success() {
        anyhow::bail!("Discogs release fetch failed ({}): {}", status, body);
    }

    let release: DiscogsRelease = serde_json::from_str(&body)
        .context("Failed to parse Discogs release")?;

    tracing::info!(
        title = %release.title,
        tracks = release.tracklist.len(),
        artists = release.artists.len(),
        "Fetched release"
    );

    Ok(release)
}

/// Look up Discogs metadata for a recording identified by the LLM.
///
/// Searches Discogs using opera title + conductor, filters/ranks results
/// to prefer CD releases matching the expected disc count, and returns
/// metadata from the best match.
pub async fn lookup_discogs_metadata(
    llm_result: &LlmRecordingSearch,
    opera: &str,
    token: &str,
) -> Result<Option<DiscogsInfo>> {
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()?;

    let opera_title = opera_slug_to_title(opera);

    // Build search query: opera title + conductor
    let mut q_parts = vec![opera_title];
    if let Some(ref conductor) = llm_result.conductor {
        q_parts.push(conductor.clone());
    }

    // Don't use year as a strict filter — Discogs release years often
    // differ from recording years (e.g., recorded 1981, released 1982/1984).
    // Instead, use label as a filter and keep the query broad.
    let mut params: Vec<(&str, String)> = vec![
        ("type", "release".to_string()),
        ("per_page", "50".to_string()),
        ("q", q_parts.join(" ")),
    ];

    if let Some(ref label) = llm_result.label {
        params.push(("label", label.clone()));
    }

    let url = format!("{DISCOGS_API_BASE}/database/search");
    tracing::info!(url = %url, query = ?q_parts, "Searching Discogs for metadata");

    let response = client
        .get(&url)
        .header("Authorization", format!("Discogs token={token}"))
        .query(&params)
        .send()
        .await
        .context("Discogs metadata search failed")?;

    let status = response.status();
    let body = response.text().await.context("Failed to read Discogs response")?;

    if !status.is_success() {
        tracing::warn!(status = %status, "Discogs metadata search failed");
        return Ok(None);
    }

    let search_resp: DiscogsSearchResponse = serde_json::from_str(&body)
        .context("Failed to parse Discogs search response")?;

    if search_resp.results.is_empty() {
        tracing::info!("No Discogs results for metadata lookup");
        return Ok(None);
    }

    tracing::info!(results = search_resp.results.len(), "Discogs metadata search results");

    // Score results: prefer CD format matching expected disc count
    let expected_discs = llm_result.disc_count.unwrap_or(0);
    let mut best: Option<(i32, &DiscogsSearchResult)> = None;

    for r in &search_resp.results {
        let mut score: i32 = 0;

        // Prefer CD over vinyl/cassette
        if let Some(ref fmts) = r.format {
            if fmts.iter().any(|f| f == "CD") {
                score += 10;
            }
            // Penalise compilations and highlights
            if fmts.iter().any(|f| f == "Compilation") {
                score -= 20;
            }
        }

        // Prefer matching disc count
        if expected_discs > 0 {
            if let Some(fq) = r.format_quantity {
                if fq == expected_discs as u32 {
                    score += 15;
                }
            }
        }

        // Prefer releases with a year
        if r.year.is_some() {
            score += 2;
        }

        // Prefer more popular releases (more "have" implies more data)
        // (community data not in search results, but catno presence is a proxy)
        if r.catno.is_some() {
            score += 1;
        }

        if best.map_or(true, |(s, _)| score > s) {
            best = Some((score, r));
        }
    }

    let (score, winner) = best.unwrap();
    tracing::info!(
        release_id = winner.id,
        catno = ?winner.catno,
        score = score,
        title = %winner.title,
        "Selected best Discogs match"
    );

    let discogs_url = winner.uri.as_deref()
        .map(|u| format!("https://www.discogs.com{u}"))
        .unwrap_or_else(|| format!("https://www.discogs.com/release/{}", winner.id));

    // Filter out empty barcodes
    let barcode = winner.barcode.as_ref()
        .map(|b| b.iter().filter(|s| !s.is_empty()).cloned().collect::<Vec<_>>())
        .filter(|b| !b.is_empty());

    Ok(Some(DiscogsInfo {
        release_id: winner.id,
        master_id: winner.master_id.filter(|&m| m > 0),
        catno: winner.catno.clone(),
        label: winner.label.clone(),
        country: winner.country.clone(),
        year: winner.year.clone(),
        format: winner.format.clone(),
        barcode,
        url: discogs_url,
    }))
}

// ---------------------------------------------------------------------------
// Duration and position parsing
// ---------------------------------------------------------------------------

/// Parse a Discogs duration string like "3:51" into seconds.
fn parse_duration(s: &str) -> Option<u64> {
    if s.is_empty() {
        return None;
    }
    let parts: Vec<&str> = s.split(':').collect();
    match parts.len() {
        2 => {
            let min: u64 = parts[0].trim().parse().ok()?;
            let sec: u64 = parts[1].trim().parse().ok()?;
            Some(min * 60 + sec)
        }
        3 => {
            let hrs: u64 = parts[0].trim().parse().ok()?;
            let min: u64 = parts[1].trim().parse().ok()?;
            let sec: u64 = parts[2].trim().parse().ok()?;
            Some(hrs * 3600 + min * 60 + sec)
        }
        _ => None,
    }
}

/// Parse a Discogs track position into (disc_number, track_number).
///
/// Handles formats:
/// - "1-1", "2-3" (multi-disc CD)
/// - "A1", "B2" (vinyl sides → disc 1=A, 2=B, etc.)
/// - "1", "2" (single disc)
fn parse_position(pos: &str) -> (Option<u32>, Option<u32>) {
    if pos.is_empty() {
        return (None, None);
    }

    // Multi-disc: "1-1", "2-3"
    if let Some((disc, track)) = pos.split_once('-') {
        let disc_num = disc.trim().parse().ok();
        let track_num = track.trim().parse().ok();
        return (disc_num, track_num);
    }

    // Vinyl sides: "A1", "B2", etc.
    let first = pos.chars().next().unwrap();
    if first.is_ascii_alphabetic() && pos.len() >= 2 {
        let disc = (first.to_ascii_uppercase() as u32) - ('A' as u32) + 1;
        let track: Option<u32> = pos[1..].parse().ok();
        return (Some(disc), track);
    }

    // Single disc: "1", "2"
    let track_num = pos.trim().parse().ok();
    (Some(1), track_num)
}

// ---------------------------------------------------------------------------
// Claude LLM search
// ---------------------------------------------------------------------------

const CLAUDE_API_URL: &str = "https://api.anthropic.com/v1/messages";

/// Response from Claude identifying the recording and suggesting sources.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LlmRecordingSearch {
    /// Full name of the recording as identified by Claude.
    #[serde(default)]
    pub recording_title: String,
    /// Conductor's full name.
    #[serde(default)]
    pub conductor: Option<String>,
    /// Orchestra / ensemble.
    #[serde(default)]
    pub orchestra: Option<String>,
    /// Record label.
    #[serde(default)]
    pub label: Option<String>,
    /// Release year.
    #[serde(default)]
    pub year: Option<u16>,
    /// Album title as it appears on the release.
    #[serde(default)]
    pub album_title: Option<String>,
    /// Number of discs in the release.
    #[serde(default)]
    pub disc_count: Option<u32>,
    /// Total number of tracks.
    #[serde(default)]
    pub track_count: Option<u32>,
    /// Wikipedia URLs that might contain track listing info.
    #[serde(default)]
    pub wikipedia_urls: Vec<String>,
    /// Discogs release or search URL if known.
    #[serde(default)]
    pub discogs_url: Option<String>,
    /// Any other useful URLs (MusicBrainz, AllMusic, etc.).
    #[serde(default)]
    pub other_urls: Vec<String>,
    /// Key artists / soloists on the recording.
    #[serde(default)]
    pub artists: Vec<String>,
    /// Free-text notes from Claude about the recording.
    #[serde(default)]
    pub notes: Option<String>,
}

/// Claude API response envelope.
#[derive(Debug, Deserialize)]
struct ClaudeResponse {
    content: Vec<ClaudeContentBlock>,
}

#[derive(Debug, Deserialize)]
struct ClaudeContentBlock {
    text: Option<String>,
}

/// Ask Claude to identify a recording and find web sources.
pub async fn search_with_llm(
    opera: &str,
    query: &str,
    api_key: &str,
    model: &str,
) -> Result<LlmRecordingSearch> {
    let opera_title = opera_slug_to_title(opera);

    let prompt = format!(
r#"I'm looking for a specific opera recording. The opera is "{opera_title}" and here is what I know about the recording:

"{query}"

Please identify this recording and provide the following information as a JSON object.

IMPORTANT for URLs: I need you to suggest the most likely Wikipedia pages where track listing information for this recording could be found. Common patterns include:
- The opera's main Wikipedia article (which often has a "Notable recordings" section)
- The conductor's discography page (e.g. "https://en.wikipedia.org/wiki/Georg_Solti_discography")
- A dedicated album article if one exists
- The opera's discography page if one exists (e.g. "https://en.wikipedia.org/wiki/Le_nozze_di_Figaro_discography")

Please include ALL plausible Wikipedia URLs in the "wikipedia_urls" array, even if you're not 100% certain they exist. I will verify them myself. Also suggest a likely Discogs search URL.

{{
  "recording_title": "Full title as commonly known",
  "conductor": "Full name of conductor",
  "orchestra": "Orchestra and/or chorus name",
  "label": "Record label",
  "year": 1981,
  "album_title": "Album title as it appears on the release",
  "disc_count": 3,
  "track_count": 32,
  "wikipedia_urls": [
    "https://en.wikipedia.org/wiki/Most_likely_page_with_track_listing",
    "https://en.wikipedia.org/wiki/Another_possible_page"
  ],
  "discogs_url": "https://www.discogs.com/search/?q=search+terms&type=release (construct a likely search URL)",
  "other_urls": ["any other useful URLs like MusicBrainz, AllMusic"],
  "artists": ["Key soloists with their roles, e.g. 'Kiri Te Kanawa (Countess)'"],
  "notes": "Any relevant notes about this recording"
}}

Respond with ONLY the JSON object, no markdown fences, no explanation."#,
        opera_title = opera_title,
        query = query,
    );

    let request_body = serde_json::json!({
        "model": model,
        "max_tokens": 2048,
        "messages": [{
            "role": "user",
            "content": prompt,
        }]
    });

    let client = reqwest::Client::new();
    tracing::info!(model = %model, "Calling Claude to identify recording");

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
    let body = response.text().await.context("Failed to read Claude response")?;

    if !status.is_success() {
        anyhow::bail!("Claude API error ({}): {}", status, body);
    }

    let claude_resp: ClaudeResponse = serde_json::from_str(&body)
        .context("Failed to parse Claude response envelope")?;

    let text = claude_resp.content.iter()
        .find_map(|b| b.text.clone())
        .ok_or_else(|| anyhow::anyhow!("No text in Claude response"))?;

    tracing::debug!(raw_response = %text, "Claude response");

    // Extract JSON from response (handle markdown fences etc.)
    let json_text = extract_json_from_response(&text)
        .ok_or_else(|| anyhow::anyhow!("No JSON found in Claude response:\n{text}"))?;

    let result: LlmRecordingSearch = serde_json::from_str(&json_text)
        .context(format!("Failed to parse LLM recording search JSON:\n{json_text}"))?;

    tracing::info!(
        recording = %result.recording_title,
        conductor = ?result.conductor,
        wikipedia_urls = ?result.wikipedia_urls,
        discogs = ?result.discogs_url,
        "Claude identified recording"
    );

    Ok(result)
}

/// Extract a JSON object from a response that may contain surrounding text.
fn extract_json_from_response(text: &str) -> Option<String> {
    let trimmed = text.trim();

    // Try markdown code fence
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

    // Find outermost { ... } by bracket counting
    let mut depth = 0i32;
    let mut start_idx = None;
    for (i, ch) in trimmed.char_indices() {
        if ch == '{' {
            if depth == 0 { start_idx = Some(i); }
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

    // Whole thing is JSON
    if serde_json::from_str::<serde_json::Value>(trimmed).is_ok() {
        return Some(trimmed.to_string());
    }

    None
}

// ---------------------------------------------------------------------------
// Wikipedia scraping
// ---------------------------------------------------------------------------

/// Result of scraping a Wikipedia recording article.
#[derive(Debug, Clone)]
pub struct WikipediaTrackListing {
    pub source_url: String,
    pub tracks: Vec<AcquiredTrackTiming>,
}

/// Try to find and parse a track listing from Wikipedia.
///
/// Checks each URL from the LLM search result. For each page:
/// 1. If it has a track listing section, parse it directly.
/// 2. If it has links to recording articles matching our opera+conductor, follow them.
/// 3. If it has links to discography pages, follow those and repeat.
pub async fn scrape_wikipedia_tracks(
    opera: &str,
    llm_result: &LlmRecordingSearch,
) -> Result<Option<WikipediaTrackListing>> {
    let client = reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .build()?;

    let opera_keywords = extract_opera_keywords(opera);
    tracing::debug!(keywords = ?opera_keywords, "Opera keywords for URL matching");

    for url in &llm_result.wikipedia_urls {
        if !url.contains("wikipedia.org/wiki/") {
            continue;
        }

        tracing::info!(url = %url, "Trying Wikipedia URL");

        let html = match fetch_wikipedia_page(&client, url).await {
            Ok(h) => h,
            Err(e) => {
                tracing::warn!(url = %url, error = %e, "Failed to fetch Wikipedia page");
                continue;
            }
        };

        // Try 1: does this page have a track listing?
        if let Some(listing) = parse_track_listing_page(&html, url) {
            return Ok(Some(listing));
        }

        // Try 2: is this a page with links to recording articles?
        if let Some(article_url) = find_recording_article_link(&html, llm_result, &opera_keywords) {
            tracing::info!(article_url = %article_url, "Found recording article link");

            if let Some(listing) = fetch_and_parse_tracks(&client, &article_url).await {
                return Ok(Some(listing));
            }
        }

        // Try 3: is this an opera article with a link to a discography page?
        if let Some(discog_url) = find_discography_page_link(&html, &opera_keywords) {
            tracing::info!(discog_url = %discog_url, "Found discography page link");

            let discog_html = match fetch_wikipedia_page(&client, &discog_url).await {
                Ok(h) => h,
                Err(e) => {
                    tracing::warn!(url = %discog_url, error = %e, "Failed to fetch discography page");
                    continue;
                }
            };

            // The discography page may have direct recording article links
            if let Some(article_url) = find_recording_article_link(&discog_html, llm_result, &opera_keywords) {
                tracing::info!(article_url = %article_url, "Found recording article on discography page");

                if let Some(listing) = fetch_and_parse_tracks(&client, &article_url).await {
                    return Ok(Some(listing));
                }
            }
        }
    }

    Ok(None)
}

/// Fetch a Wikipedia page and try to parse its track listing.
async fn fetch_and_parse_tracks(
    client: &reqwest::Client,
    url: &str,
) -> Option<WikipediaTrackListing> {
    let html = match fetch_wikipedia_page(client, url).await {
        Ok(h) => h,
        Err(e) => {
            tracing::warn!(url = %url, error = %e, "Failed to fetch page");
            return None;
        }
    };
    parse_track_listing_page(&html, url)
}

/// Extract keywords from an opera slug for matching against URLs.
///
/// From "mozart/le-nozze-di-figaro" extracts meaningful words like
/// ["nozze", "figaro"] (skipping short words and composer).
fn extract_opera_keywords(opera: &str) -> Vec<String> {
    let slug = opera.rsplit('/').next().unwrap_or(opera);
    let sep = if slug.contains('_') { '_' } else { '-' };
    slug.split(sep)
        .map(|w| w.to_lowercase())
        .filter(|w| w.len() > 2) // skip "le", "di", "la", etc.
        .filter(|w| !matches!(w.as_str(), "the", ))
        .collect()
}

async fn fetch_wikipedia_page(client: &reqwest::Client, url: &str) -> Result<String> {
    let response = client.get(url)
        .send()
        .await
        .context("Failed to fetch Wikipedia page")?;

    let status = response.status();
    if !status.is_success() {
        anyhow::bail!("Wikipedia returned status {}", status);
    }

    response.text().await.context("Failed to read Wikipedia page body")
}

/// Parse a Wikipedia recording article page for track listings.
///
/// Handles two Wikipedia HTML formats:
/// - **New** (Vector 2022): `<div class="mw-heading mw-heading2"><h2>Track listing, CD 1</h2></div>`
/// - **Old**: `<h2><span class="mw-headline">Track listing, CD 1</span></h2>`
///
/// After each heading, walks siblings looking for `<ul>/<ol>` lists
/// where each `<li>` has format: `{track} ({M:SS}) {title}`.
fn parse_track_listing_page(html: &str, source_url: &str) -> Option<WikipediaTrackListing> {
    let document = Html::parse_document(html);

    let h2_sel = Selector::parse("h2").ok()?;
    let li_sel = Selector::parse("li").ok()?;

    let mut tracks = Vec::new();
    let mut found_track_listing = false;

    for h2 in document.select(&h2_sel) {
        let h2_text: String = h2.text().collect();
        // Filter out [edit] link text
        let h2_clean = h2_text.replace("[edit]", "").trim().to_string();

        if !h2_clean.contains("Track listing") {
            continue;
        }

        found_track_listing = true;
        let current_disc = extract_disc_from_header(&h2_clean);
        tracing::debug!(header = %h2_clean, disc = ?current_disc, "Found track listing header");

        // The h2 may be wrapped in <div class="mw-heading">. Walk siblings
        // from the parent wrapper if it exists, otherwise from the h2 itself.
        let walk_from = h2.parent()
            .and_then(ElementRef::wrap)
            .filter(|parent| {
                parent.value().classes().any(|c| c.starts_with("mw-heading"))
            })
            .map(|parent| parent.id())
            .unwrap_or_else(|| h2.id());

        let walk_node = document.tree.get(walk_from).unwrap();
        let mut sibling = walk_node.next_sibling();

        while let Some(node) = sibling {
            if let Some(el) = ElementRef::wrap(node) {
                let tag = el.value().name();
                // Stop at next heading wrapper or heading element
                if tag == "h2" || tag == "h3" {
                    break;
                }
                if tag == "div" && el.value().classes().any(|c| c.starts_with("mw-heading")) {
                    break;
                }
                // Collect tracks from list elements
                // el.select(&li_sel) finds <li> inside <ul>, <ol>, or <div> wrappers
                for li in el.select(&li_sel) {
                    let li_text = li.text().collect::<String>();
                    if let Some(track) = parse_wikipedia_track_line(&li_text, current_disc) {
                        tracks.push(track);
                    }
                }
            }
            sibling = node.next_sibling();
        }
    }

    if found_track_listing && !tracks.is_empty() {
        tracing::info!(
            tracks = tracks.len(),
            source = %source_url,
            "Parsed Wikipedia track listing"
        );
        Some(WikipediaTrackListing {
            source_url: source_url.to_string(),
            tracks,
        })
    } else {
        if found_track_listing {
            tracing::warn!(source = %source_url, "Found Track listing header but no parseable tracks");
        }
        None
    }
}

/// Extract disc number from a header like "Track listing, CD 1".
fn extract_disc_from_header(header: &str) -> Option<u32> {
    let lower = header.to_lowercase();
    // "CD N"
    if let Some(idx) = lower.find("cd ") {
        let after = &header[idx + 3..];
        let num_str: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
        if let Ok(n) = num_str.parse() {
            return Some(n);
        }
    }
    // "Disc N"
    if let Some(idx) = lower.find("disc ") {
        let after = &header[idx + 5..];
        let num_str: String = after.chars().take_while(|c| c.is_ascii_digit()).collect();
        if let Ok(n) = num_str.parse() {
            return Some(n);
        }
    }
    // Plain "Track listing" with no disc indicator → disc 1
    if !lower.contains("cd ") && !lower.contains("disc ") {
        return Some(1);
    }
    None
}

/// Parse a Wikipedia track line like:
///   `1 (3:51) Sinfonia`
///   `2 (3:24) No. 1 Duetto "Cinque...dieci..." (Figaro, Susanna); recitativo ...`
fn parse_wikipedia_track_line(line: &str, disc: Option<u32>) -> Option<AcquiredTrackTiming> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    // Find the first digit sequence (track number)
    let digit_start = trimmed.find(|c: char| c.is_ascii_digit())?;
    let after_start = &trimmed[digit_start..];
    let digit_end = after_start
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(after_start.len());
    let track_str = &after_start[..digit_end];
    let track_number: u32 = track_str.parse().ok()?;

    let rest = after_start[digit_end..].trim_start();

    // Expect '(' for duration
    if !rest.starts_with('(') {
        return None;
    }
    let close_paren = rest.find(')')?;
    let duration_str = &rest[1..close_paren];
    let duration_seconds = parse_duration(duration_str);

    // Title is everything after the closing paren
    let title = rest[close_paren + 1..].trim().to_string();
    if title.is_empty() {
        return None;
    }

    let characters = libretto_model::resolve::extract_characters_from_title(&title);
    Some(AcquiredTrackTiming {
        track_title: title,
        disc_number: disc,
        track_number: Some(track_number),
        duration_seconds,
        characters,
        number_ids: vec![],
    })
}

/// Find a link to a specific recording article.
///
/// Looks for `<a>` elements whose href contains "_recording)" and
/// matches BOTH the opera keywords AND the conductor/year from the LLM result.
fn find_recording_article_link(
    html: &str,
    llm_result: &LlmRecordingSearch,
    opera_keywords: &[String],
) -> Option<String> {
    let document = Html::parse_document(html);
    let a_sel = Selector::parse("a").ok()?;

    let conductor = llm_result.conductor.as_deref().unwrap_or("");
    let year_str = llm_result.year.map(|y| y.to_string()).unwrap_or_default();

    // Conductor's last name for matching
    let conductor_last = conductor
        .split_whitespace()
        .last()
        .unwrap_or("")
        .to_lowercase();

    for a in document.select(&a_sel) {
        if let Some(href) = a.value().attr("href") {
            let href_lower = href.to_lowercase();

            // Recording article links look like:
            // /wiki/Le_nozze_di_Figaro_(Georg_Solti_recording)
            if !href_lower.contains("recording)") {
                continue;
            }

            // Must match at least one opera keyword to avoid wrong-opera matches
            let matches_opera = opera_keywords.iter()
                .any(|kw| href_lower.contains(kw.as_str()));
            if !matches_opera {
                continue;
            }

            let matches_conductor =
                conductor_last.len() > 2 && href_lower.contains(&conductor_last);
            let matches_year =
                !year_str.is_empty() && href_lower.contains(&year_str);

            if matches_conductor || matches_year {
                if href.starts_with("/wiki/") {
                    return Some(format!("https://en.wikipedia.org{}", href));
                } else if href.starts_with("https://") {
                    return Some(href.to_string());
                }
            }
        }
    }

    None
}

/// Find a link to a discography page from an opera article.
///
/// Looks for `<a>` elements whose href contains "discography" and
/// matches opera keywords.
fn find_discography_page_link(html: &str, opera_keywords: &[String]) -> Option<String> {
    let document = Html::parse_document(html);
    let a_sel = Selector::parse("a").ok()?;

    for a in document.select(&a_sel) {
        if let Some(href) = a.value().attr("href") {
            let href_lower = href.to_lowercase();

            if !href_lower.contains("discography") {
                continue;
            }
            // Must also relate to our opera
            let matches_opera = opera_keywords.iter()
                .any(|kw| href_lower.contains(kw.as_str()));
            // Also accept if the link text mentions the opera
            let link_text: String = a.text().collect::<String>().to_lowercase();
            let text_matches = opera_keywords.iter()
                .any(|kw| link_text.contains(kw.as_str()));

            if matches_opera || text_matches {
                if href.starts_with("/wiki/") {
                    return Some(format!("https://en.wikipedia.org{}", href));
                } else if href.starts_with("https://") {
                    return Some(href.to_string());
                }
            }
        }
    }

    None
}

// ---------------------------------------------------------------------------
// Output types
// ---------------------------------------------------------------------------

/// Complete output of recording acquisition.
#[derive(Debug, Clone, Serialize)]
pub struct AcquiredRecording {
    pub version: String,
    pub base_libretto: String,
    pub recording: AcquiredRecordingMetadata,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub discogs: Option<DiscogsInfo>,
    pub contributors: Vec<AcquiredContributor>,
    pub track_timings: Vec<AcquiredTrackTiming>,
    /// Other Discogs releases that matched the search query.
    pub _search_alternatives: Vec<SearchAlternative>,
}

/// Discogs metadata for the recording.
#[derive(Debug, Clone, Serialize)]
pub struct DiscogsInfo {
    pub release_id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub master_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub catno: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub year: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub barcode: Option<Vec<String>>,
    pub url: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct AcquiredRecordingMetadata {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub conductor: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub orchestra: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub year: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub album_title: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub album_title_aliases: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AcquiredContributor {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AcquiredTrackTiming {
    pub track_title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disc_number: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub track_number: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_seconds: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub characters: Vec<String>,
    pub number_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchAlternative {
    pub discogs_id: u64,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub year: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub format: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
}

// ---------------------------------------------------------------------------
// Main entry point
// ---------------------------------------------------------------------------

/// Acquire recording track listing using LLM search + Wikipedia scraping.
///
/// Uses Claude to identify the recording and find Wikipedia URLs, then
/// scrapes the track listing from the Wikipedia recording article.
/// If a Discogs token is provided, also looks up catalog/label metadata.
pub async fn acquire_recording_from_wikipedia(
    opera: &str,
    llm_result: &LlmRecordingSearch,
    output_dir: &str,
    discogs_token: Option<&str>,
) -> Result<()> {
    // Try to scrape track listing from Wikipedia
    let wiki_result = scrape_wikipedia_tracks(opera, llm_result).await?;

    let wiki_listing = wiki_result
        .ok_or_else(|| anyhow::anyhow!(
            "No track listing found on any Wikipedia URL. Tried: {:?}",
            llm_result.wikipedia_urls
        ))?;

    // Look up Discogs metadata if token is available
    let discogs_info = if let Some(token) = discogs_token {
        match lookup_discogs_metadata(llm_result, opera, token).await {
            Ok(info) => info,
            Err(e) => {
                tracing::warn!(error = %e, "Discogs metadata lookup failed, continuing without");
                None
            }
        }
    } else {
        tracing::info!("No DISCOGS_TOKEN — skipping Discogs metadata lookup");
        None
    };

    // Build output using LLM metadata + Wikipedia tracks + Discogs metadata
    let now = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let output = AcquiredRecording {
        version: "1.0".to_string(),
        base_libretto: String::new(),
        recording: AcquiredRecordingMetadata {
            conductor: llm_result.conductor.clone(),
            orchestra: llm_result.orchestra.clone(),
            year: llm_result.year,
            label: llm_result.label.clone(),
            album_title: llm_result.album_title.clone(),
            album_title_aliases: vec![],
        },
        discogs: discogs_info,
        contributors: build_contributors_from_llm(llm_result, &wiki_listing.source_url, &now),
        track_timings: wiki_listing.tracks,
        _search_alternatives: vec![],
    };

    // Write output
    let dir = Path::new(output_dir);
    std::fs::create_dir_all(dir)?;

    let slug = generate_filename_slug(
        llm_result.conductor.as_deref(),
        llm_result.year,
    );
    let filename = format!("{slug}.timing.json");
    let path = dir.join(&filename);

    let json = serde_json::to_string_pretty(&output)?;
    std::fs::write(&path, &json)?;
    tracing::info!(
        path = %path.display(),
        tracks = output.track_timings.len(),
        source = %wiki_listing.source_url,
        "Wrote recording timing JSON from Wikipedia"
    );

    Ok(())
}

/// Build contributor list from LLM search result.
fn build_contributors_from_llm(
    llm: &LlmRecordingSearch,
    source_url: &str,
    date: &str,
) -> Vec<AcquiredContributor> {
    let mut contributors = Vec::new();

    // Source attribution
    contributors.push(AcquiredContributor {
        name: "Wikipedia".to_string(),
        role: Some("track listing source".to_string()),
        date: Some(format!("{date} ({source_url})")),
    });

    // Artists from the LLM result
    for artist_str in &llm.artists {
        // Parse "Name (Role)" format
        if let Some(paren_start) = artist_str.rfind('(') {
            let name = artist_str[..paren_start].trim().to_string();
            let role = artist_str[paren_start + 1..]
                .trim_end_matches(')')
                .trim()
                .to_string();
            contributors.push(AcquiredContributor {
                name,
                role: Some(role),
                date: None,
            });
        } else {
            contributors.push(AcquiredContributor {
                name: artist_str.clone(),
                role: None,
                date: None,
            });
        }
    }

    contributors
}

/// Acquire recording track listing from Discogs.
///
/// Searches Discogs for the recording described by the free-text query,
/// fetches the full track listing, and writes a timing.json file.
pub async fn acquire_recording(
    opera: &str,
    query: &str,
    output_dir: &str,
    discogs_token: &str,
) -> Result<()> {
    let parsed = parse_query(query);
    tracing::info!(
        conductor = ?parsed.conductor,
        year = ?parsed.year,
        label = ?parsed.label,
        orchestra = ?parsed.orchestra,
        artists = ?parsed.artists,
        "Parsed recording query"
    );

    let opera_title = opera_slug_to_title(opera);
    tracing::info!(opera_title = %opera_title, "Searching for recording");

    // Search Discogs
    let results = search_discogs(&parsed, &opera_title, discogs_token).await?;

    if results.is_empty() {
        anyhow::bail!("No Discogs results found for query: {query}");
    }

    // Select best match (first result) and collect alternatives
    let best = &results[0];
    tracing::info!(
        id = best.id,
        title = %best.title,
        year = ?best.year,
        "Selected best match"
    );

    let alternatives: Vec<SearchAlternative> = results.iter().skip(1).map(|r| {
        SearchAlternative {
            discogs_id: r.id,
            title: r.title.clone(),
            year: r.year.clone(),
            label: r.label.clone(),
            format: r.format.clone(),
            country: r.country.clone(),
        }
    }).collect();

    // Fetch full release details
    let release = fetch_release(best.id, discogs_token).await?;

    // Extract metadata — prefer Discogs data, fall back to parsed query
    let conductor = extract_conductor(&release)
        .or_else(|| parsed.conductor.clone());

    let orchestra = parsed.orchestra.clone()
        .or_else(|| extract_orchestra(&release));

    let label = release.labels.first().map(|l| l.name.clone())
        .or(parsed.label.clone());

    let year = release.year.or(parsed.year);
    let album_title = Some(release.title.clone());

    // Build track timings — skip headings (disc/act separators)
    let track_timings: Vec<AcquiredTrackTiming> = release.tracklist.iter()
        .filter(|t| t.type_ != "heading")
        .map(|t| {
            let (disc, track) = parse_position(&t.position);
            let duration = parse_duration(&t.duration);
            AcquiredTrackTiming {
                track_title: t.title.clone(),
                disc_number: disc,
                track_number: track,
                duration_seconds: duration,
                characters: libretto_model::resolve::extract_characters_from_title(&t.title),
                number_ids: vec![],
            }
        })
        .collect();

    tracing::info!(tracks = track_timings.len(), "Built track timings");

    // Build output
    let now = chrono::Utc::now().format("%Y-%m-%d").to_string();
    let output = AcquiredRecording {
        version: "1.0".to_string(),
        base_libretto: String::new(),
        recording: AcquiredRecordingMetadata {
            conductor: conductor.clone(),
            orchestra,
            year,
            label,
            album_title,
            album_title_aliases: vec![],
        },
        discogs: {
            let discogs_url = best.uri.as_deref()
                .map(|u| format!("https://www.discogs.com{u}"))
                .unwrap_or_else(|| format!("https://www.discogs.com/release/{}", best.id));
            let barcode = best.barcode.as_ref()
                .map(|b| b.iter().filter(|s| !s.is_empty()).cloned().collect::<Vec<_>>())
                .filter(|b| !b.is_empty());
            Some(DiscogsInfo {
                release_id: best.id,
                master_id: best.master_id.filter(|&m| m > 0),
                catno: best.catno.clone(),
                label: best.label.clone(),
                country: best.country.clone(),
                year: best.year.clone(),
                format: best.format.clone(),
                barcode,
                url: discogs_url,
            })
        },
        contributors: vec![AcquiredContributor {
            name: "Discogs".to_string(),
            role: Some("track listing source".to_string()),
            date: Some(now),
        }],
        track_timings,
        _search_alternatives: alternatives,
    };

    // Write output
    let dir = Path::new(output_dir);
    std::fs::create_dir_all(dir)?;

    let slug = generate_filename_slug(
        conductor.as_deref(),
        year,
    );
    let filename = format!("{slug}.timing.json");
    let path = dir.join(&filename);

    let json = serde_json::to_string_pretty(&output)?;
    std::fs::write(&path, &json)?;
    tracing::info!(
        path = %path.display(),
        tracks = output.track_timings.len(),
        alternatives = output._search_alternatives.len(),
        "Wrote recording timing JSON"
    );

    Ok(())
}

/// Try to extract the conductor from Discogs artist credits.
fn extract_conductor(release: &DiscogsRelease) -> Option<String> {
    // Look for an artist with a conductor-related role
    for artist in &release.artists {
        if let Some(ref role) = artist.role {
            let lower = role.to_lowercase();
            if lower.contains("conduct") || lower.contains("dirigent") {
                return Some(clean_discogs_name(&artist.name));
            }
        }
    }
    None
}

/// Try to extract the orchestra from Discogs artist credits.
fn extract_orchestra(release: &DiscogsRelease) -> Option<String> {
    for artist in &release.artists {
        let lower = artist.name.to_lowercase();
        if contains_ensemble_keyword(&lower) {
            return Some(clean_discogs_name(&artist.name));
        }
    }
    None
}

/// Discogs appends " (N)" to artist names for disambiguation — strip that.
fn clean_discogs_name(name: &str) -> String {
    // Remove trailing " (N)" where N is a number
    if let Some(idx) = name.rfind(" (") {
        let suffix = &name[idx + 2..];
        if suffix.ends_with(')') && suffix[..suffix.len()-1].chars().all(|c| c.is_ascii_digit()) {
            return name[..idx].to_string();
        }
    }
    name.to_string()
}

/// Generate a slug for the output filename from conductor + year.
fn generate_filename_slug(conductor: Option<&str>, year: Option<u16>) -> String {
    let conductor_part = conductor.unwrap_or("unknown");
    let year_part = year.map(|y| y.to_string()).unwrap_or_else(|| "unknown".to_string());
    let raw = format!("{}-{}", conductor_part, year_part);

    let mut slug = String::new();
    let mut prev_dash = false;
    for c in raw.chars() {
        if c.is_alphanumeric() {
            slug.push(c.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            slug.push('-');
            prev_dash = true;
        }
    }
    slug.trim_matches('-').to_string()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_query_full() {
        let q = parse_query("solti, 1981, decca, london philharmonic orchestra and london philharmonic chorus, with Kiri Te Kanawa");
        assert_eq!(q.conductor.as_deref(), Some("solti"));
        assert_eq!(q.year, Some(1981));
        assert_eq!(q.label.as_deref(), Some("decca"));
        assert!(q.orchestra.as_ref().unwrap().contains("london philharmonic orchestra"));
        assert_eq!(q.artists, vec!["Kiri Te Kanawa"]);
    }

    #[test]
    fn test_parse_query_minimal() {
        let q = parse_query("karajan, 1978, dg");
        assert_eq!(q.conductor.as_deref(), Some("karajan"));
        assert_eq!(q.year, Some(1978));
        assert_eq!(q.label.as_deref(), Some("dg"));
        assert!(q.orchestra.is_none());
        assert!(q.artists.is_empty());
    }

    #[test]
    fn test_parse_duration() {
        assert_eq!(parse_duration("3:51"), Some(231));
        assert_eq!(parse_duration("0:45"), Some(45));
        assert_eq!(parse_duration("10:02"), Some(602));
        assert_eq!(parse_duration("1:15:30"), Some(4530));
        assert_eq!(parse_duration(""), None);
        assert_eq!(parse_duration("invalid"), None);
    }

    #[test]
    fn test_parse_position_multi_disc() {
        assert_eq!(parse_position("1-1"), (Some(1), Some(1)));
        assert_eq!(parse_position("2-3"), (Some(2), Some(3)));
        assert_eq!(parse_position("3-10"), (Some(3), Some(10)));
    }

    #[test]
    fn test_parse_position_vinyl() {
        assert_eq!(parse_position("A1"), (Some(1), Some(1)));
        assert_eq!(parse_position("B3"), (Some(2), Some(3)));
        assert_eq!(parse_position("C1"), (Some(3), Some(1)));
    }

    #[test]
    fn test_parse_position_single_disc() {
        assert_eq!(parse_position("1"), (Some(1), Some(1)));
        assert_eq!(parse_position("12"), (Some(1), Some(12)));
    }

    #[test]
    fn test_opera_slug_to_title() {
        assert_eq!(opera_slug_to_title("mozart/le-nozze-di-figaro"), "Le Nozze Di Figaro");
        assert_eq!(opera_slug_to_title("le-nozze-di-figaro"), "Le Nozze Di Figaro");
        assert_eq!(opera_slug_to_title("Le_nozze_di_Figaro"), "Le Nozze Di Figaro");
    }

    #[test]
    fn test_generate_filename_slug() {
        assert_eq!(generate_filename_slug(Some("Georg Solti"), Some(1981)), "georg-solti-1981");
        assert_eq!(generate_filename_slug(Some("Karajan"), Some(1978)), "karajan-1978");
        assert_eq!(generate_filename_slug(None, Some(1981)), "unknown-1981");
        assert_eq!(generate_filename_slug(Some("Solti"), None), "solti-unknown");
    }

    #[test]
    fn test_clean_discogs_name() {
        assert_eq!(clean_discogs_name("Georg Solti"), "Georg Solti");
        assert_eq!(clean_discogs_name("Georg Solti (2)"), "Georg Solti");
        assert_eq!(clean_discogs_name("London Philharmonic Orchestra"), "London Philharmonic Orchestra");
    }

    #[test]
    fn test_extract_disc_from_header() {
        assert_eq!(extract_disc_from_header("Track listing, CD 1"), Some(1));
        assert_eq!(extract_disc_from_header("Track listing, CD 3"), Some(3));
        assert_eq!(extract_disc_from_header("Track listing, Disc 2"), Some(2));
        assert_eq!(extract_disc_from_header("Track listing"), Some(1));
    }

    #[test]
    fn test_parse_wikipedia_track_line_simple() {
        let t = parse_wikipedia_track_line("1 (3:51) Sinfonia", Some(1)).unwrap();
        assert_eq!(t.track_number, Some(1));
        assert_eq!(t.duration_seconds, Some(231));
        assert_eq!(t.track_title, "Sinfonia");
        assert_eq!(t.disc_number, Some(1));
    }

    #[test]
    fn test_parse_wikipedia_track_line_complex() {
        let line = r#"2 (3:24) No. 1 Duetto "Cinque...dieci...venti" (Figaro, Susanna); recitativo "Cosa stai misurando" (Susanna, Figaro)"#;
        let t = parse_wikipedia_track_line(line, Some(1)).unwrap();
        assert_eq!(t.track_number, Some(2));
        assert_eq!(t.duration_seconds, Some(204));
        assert!(t.track_title.starts_with("No. 1 Duetto"));
    }

    #[test]
    fn test_parse_wikipedia_track_line_long_duration() {
        let t = parse_wikipedia_track_line("4 (10:34) No. 28 Finale, Part 1", Some(3)).unwrap();
        assert_eq!(t.track_number, Some(4));
        assert_eq!(t.duration_seconds, Some(634));
        assert_eq!(t.disc_number, Some(3));
    }

    #[test]
    fn test_parse_wikipedia_track_line_empty() {
        assert!(parse_wikipedia_track_line("", Some(1)).is_none());
        assert!(parse_wikipedia_track_line("Act One", Some(1)).is_none());
    }

    #[test]
    fn test_parse_track_listing_html() {
        let html = r#"
        <html><body>
        <h2><span class="mw-headline" id="Track_listing.2C_CD_1">Track listing, CD 1</span></h2>
        <p>Some intro text</p>
        <ul>
        <li>1 (3:51) Sinfonia</li>
        <li>2 (3:24) No. 1 Duetto "Cinque...dieci...venti" (Figaro, Susanna)</li>
        <li>3 (4:01) No. 2 Duetto "Se a caso madama" (Figaro, Susanna)</li>
        </ul>
        <h2><span class="mw-headline" id="Track_listing.2C_CD_2">Track listing, CD 2</span></h2>
        <ul>
        <li>1 (1:18) Recitativo "Che novità!" (Conte, Contessa)</li>
        <li>2 (3:49) No. 13 Terzetto "Susanna or via sortite"</li>
        </ul>
        <h2><span class="mw-headline" id="Personnel">Personnel</span></h2>
        </body></html>
        "#;

        let listing = parse_track_listing_page(html, "https://test.example.com").unwrap();
        assert_eq!(listing.tracks.len(), 5);

        // CD 1 tracks
        assert_eq!(listing.tracks[0].disc_number, Some(1));
        assert_eq!(listing.tracks[0].track_number, Some(1));
        assert_eq!(listing.tracks[0].duration_seconds, Some(231));
        assert_eq!(listing.tracks[0].track_title, "Sinfonia");

        assert_eq!(listing.tracks[2].track_number, Some(3));
        assert_eq!(listing.tracks[2].duration_seconds, Some(241));

        // CD 2 tracks
        assert_eq!(listing.tracks[3].disc_number, Some(2));
        assert_eq!(listing.tracks[3].track_number, Some(1));
        assert_eq!(listing.tracks[3].duration_seconds, Some(78));

        assert_eq!(listing.tracks[4].disc_number, Some(2));
        assert_eq!(listing.tracks[4].track_number, Some(2));
    }

    #[test]
    fn test_find_recording_article_link() {
        let html = r#"
        <html><body>
        <a href="/wiki/Some_other_page">Other</a>
        <a href="/wiki/Le_nozze_di_Figaro_(Georg_Solti_recording)">1981</a>
        <a href="/wiki/Le_nozze_di_Figaro_(Karajan_recording)">1978</a>
        </body></html>
        "#;

        let llm = LlmRecordingSearch {
            recording_title: String::new(),
            conductor: Some("Georg Solti".to_string()),
            orchestra: None,
            label: None,
            year: Some(1981),
            album_title: None,
            disc_count: None,
            track_count: None,
            wikipedia_urls: vec![],
            discogs_url: None,
            other_urls: vec![],
            artists: vec![],
            notes: None,
        };

        let keywords = extract_opera_keywords("mozart/le-nozze-di-figaro");
        let link = find_recording_article_link(html, &llm, &keywords).unwrap();
        assert_eq!(link, "https://en.wikipedia.org/wiki/Le_nozze_di_Figaro_(Georg_Solti_recording)");
    }

    #[test]
    fn test_find_recording_article_link_filters_wrong_opera() {
        // Solti discography page has many recording links — must pick the right opera
        let html = r#"
        <html><body>
        <a href="/wiki/La_damnation_de_Faust_(Georg_Solti_recording)">Faust</a>
        <a href="/wiki/Otello_(Georg_Solti_recording)">Otello</a>
        <a href="/wiki/Le_nozze_di_Figaro_(Georg_Solti_recording)">Figaro</a>
        </body></html>
        "#;

        let llm = LlmRecordingSearch {
            recording_title: String::new(),
            conductor: Some("Georg Solti".to_string()),
            orchestra: None,
            label: None,
            year: Some(1981),
            album_title: None,
            disc_count: None,
            track_count: None,
            wikipedia_urls: vec![],
            discogs_url: None,
            other_urls: vec![],
            artists: vec![],
            notes: None,
        };

        let keywords = extract_opera_keywords("mozart/le-nozze-di-figaro");
        let link = find_recording_article_link(html, &llm, &keywords).unwrap();
        assert_eq!(link, "https://en.wikipedia.org/wiki/Le_nozze_di_Figaro_(Georg_Solti_recording)");
    }

    #[test]
    fn test_extract_opera_keywords() {
        let kw = extract_opera_keywords("mozart/le-nozze-di-figaro");
        assert!(kw.contains(&"nozze".to_string()));
        assert!(kw.contains(&"figaro".to_string()));
        assert!(!kw.contains(&"le".to_string())); // too short
        assert!(!kw.contains(&"di".to_string())); // too short
    }

    #[test]
    fn test_find_discography_page_link() {
        let html = r#"
        <html><body>
        <a href="/wiki/Georg_Solti_discography">Solti discography</a>
        <a href="/wiki/The_Marriage_of_Figaro_discography">The Marriage of Figaro discography</a>
        </body></html>
        "#;

        let keywords = extract_opera_keywords("mozart/le-nozze-di-figaro");
        let link = find_discography_page_link(html, &keywords).unwrap();
        // Should match the Figaro discography, not Solti's
        assert_eq!(link, "https://en.wikipedia.org/wiki/The_Marriage_of_Figaro_discography");
    }
}
