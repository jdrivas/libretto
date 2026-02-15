use anyhow::Result;

pub mod cast;
pub mod structure;
pub mod segments;
pub mod align;

/// Parse raw libretto text files into a structured base libretto JSON.
///
/// Reads `italian.txt` and `english.txt` from the input directory,
/// parses them into structured data, and writes a `BaseLibretto` JSON file.
pub fn parse(_input_dir: &str, _output_file: &str) -> Result<()> {
    anyhow::bail!("Parser not yet implemented (Phase 3)")
}
