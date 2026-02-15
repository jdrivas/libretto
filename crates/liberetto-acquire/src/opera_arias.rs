use anyhow::Result;

/// Acquire libretto text from opera-arias.com.
///
/// Fetches the Italian and/or English libretto pages, parses the HTML,
/// extracts the libretto text, and writes clean `.txt` files.
pub async fn acquire(_opera: &str, _lang: &str, _output_dir: &str) -> Result<()> {
    anyhow::bail!("opera-arias.com adapter not yet implemented (Phase 2)")
}
