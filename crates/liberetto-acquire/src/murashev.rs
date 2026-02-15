use anyhow::Result;

/// Acquire libretto text from murashev.com.
///
/// Fetches the side-by-side bilingual page, parses the HTML table,
/// extracts pre-aligned paragraph pairs, and writes `.txt` files
/// plus a `bilingual.json` with the aligned pairs.
pub async fn acquire(_opera: &str, _lang: &str, _output_dir: &str) -> Result<()> {
    anyhow::bail!("murashev.com adapter not yet implemented (Phase 2)")
}
