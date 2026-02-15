use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

use libretto_acquire::types::{AcquiredLibretto, AcquiredMonolingual};
use libretto_model::base_libretto::{BaseLibretto, MusicalNumber, OperaMetadata};

pub mod cast;
pub mod structure;
pub mod segments;
pub mod align;

/// Parse acquired libretto files into a structured base libretto JSON.
///
/// Reads structured JSON from the input directory (bilingual.json or {lang}.json),
/// runs the parse pipeline, and writes a `BaseLibretto` JSON file.
///
/// Supported input configurations:
/// - `bilingual.json` — bilingual acquisition (produces aligned original + translation)
/// - `italian.json` + `english.json` — two monolingual files (aligned by structure)
/// - `italian.json` or `english.json` — single language (no translation)
pub fn parse(input_dir: &str, output_file: &str) -> Result<()> {
    let dir = Path::new(input_dir);

    let bilingual_path = dir.join("bilingual.json");
    let italian_json = dir.join("italian.json");
    let english_json = dir.join("english.json");

    let libretto = if bilingual_path.exists() {
        tracing::info!("Found bilingual.json — using bilingual mode");
        parse_bilingual(&bilingual_path)?
    } else if italian_json.exists() && english_json.exists() {
        tracing::info!("Found italian.json + english.json — using dual monolingual mode");
        parse_dual_monolingual(&italian_json, &english_json)?
    } else if italian_json.exists() {
        tracing::info!("Found italian.json — single language mode");
        parse_single_monolingual(&italian_json)?
    } else if english_json.exists() {
        tracing::info!("Found english.json — single language mode");
        parse_single_monolingual(&english_json)?
    } else {
        anyhow::bail!(
            "No recognized input files in {input_dir}. \
             Expected bilingual.json, italian.json, or english.json."
        );
    };

    let json = serde_json::to_string_pretty(&libretto)?;
    fs::write(output_file, &json)?;
    tracing::info!(
        path = %output_file,
        numbers = libretto.numbers.len(),
        segments = libretto.segment_ids().len(),
        "Wrote base libretto JSON"
    );

    Ok(())
}

/// Parse from a bilingual.json file.
fn parse_bilingual(path: &Path) -> Result<BaseLibretto> {
    let text = fs::read_to_string(path).context("Failed to read bilingual.json")?;
    let acquired: AcquiredLibretto = serde_json::from_str(&text)
        .context("Failed to parse bilingual.json")?;

    let (lang1_elements, lang2_elements) = align::parse_bilingual(&acquired);

    // Determine which is the original language (Italian) and which is translation
    let (original_elements, translation_elements, orig_lang, trans_lang) =
        if acquired.lang2 == "it" {
            (lang2_elements, lang1_elements, &acquired.lang2, &acquired.lang1)
        } else {
            (lang1_elements, lang2_elements, &acquired.lang1, &acquired.lang2)
        };

    tracing::info!(
        original = %orig_lang,
        translation = %trans_lang,
        orig_elements = original_elements.len(),
        trans_elements = translation_elements.len(),
        "Running bilingual pipeline"
    );

    // Run pipeline on both languages
    let orig_result = align::pipeline(&original_elements);
    let trans_result = align::pipeline(&translation_elements);

    tracing::info!(
        orig_segments = orig_result.segments.len(),
        trans_segments = trans_result.segments.len(),
        "Parsed both languages"
    );

    // Align translations into original segments
    let mut segments = orig_result.segments;
    align::align_segments(&mut segments, &trans_result.segments);

    let aligned_count = segments.iter().filter(|s| s.translation.is_some()).count();
    tracing::info!(aligned = aligned_count, total = segments.len(), "Aligned translations");

    // Build the BaseLibretto
    let metadata = OperaMetadata {
        title: acquired.source.opera.clone(),
        composer: String::new(),
        librettist: None,
        language: orig_lang.clone(),
        translation_language: Some(trans_lang.clone()),
        year: None,
    };

    assemble(metadata, &orig_result.cast, &orig_result.numbers, segments)
}

/// Parse from two separate monolingual JSON files.
fn parse_dual_monolingual(italian_path: &Path, english_path: &Path) -> Result<BaseLibretto> {
    let it_text = fs::read_to_string(italian_path).context("Failed to read italian.json")?;
    let it_acquired: AcquiredMonolingual = serde_json::from_str(&it_text)
        .context("Failed to parse italian.json")?;

    let en_text = fs::read_to_string(english_path).context("Failed to read english.json")?;
    let en_acquired: AcquiredMonolingual = serde_json::from_str(&en_text)
        .context("Failed to parse english.json")?;

    let it_result = align::pipeline(&it_acquired.elements);
    let en_result = align::pipeline(&en_acquired.elements);

    tracing::info!(
        it_segments = it_result.segments.len(),
        en_segments = en_result.segments.len(),
        "Parsed both languages"
    );

    let mut segments = it_result.segments;
    align::align_segments(&mut segments, &en_result.segments);

    let metadata = OperaMetadata {
        title: it_acquired.source.opera.clone(),
        composer: String::new(),
        librettist: None,
        language: "it".to_string(),
        translation_language: Some("en".to_string()),
        year: None,
    };

    assemble(metadata, &it_result.cast, &it_result.numbers, segments)
}

/// Parse from a single monolingual JSON file.
fn parse_single_monolingual(path: &Path) -> Result<BaseLibretto> {
    let text = fs::read_to_string(path).context("Failed to read monolingual JSON")?;
    let acquired: AcquiredMonolingual = serde_json::from_str(&text)
        .context("Failed to parse monolingual JSON")?;

    let result = align::pipeline(&acquired.elements);

    tracing::info!(
        lang = %acquired.lang,
        segments = result.segments.len(),
        "Parsed single language"
    );

    let metadata = OperaMetadata {
        title: acquired.source.opera.clone(),
        composer: String::new(),
        librettist: None,
        language: acquired.lang.clone(),
        translation_language: None,
        year: None,
    };

    assemble(metadata, &result.cast, &result.numbers, result.segments)
}

/// Assemble a BaseLibretto from pipeline results.
fn assemble(
    metadata: OperaMetadata,
    cast_members: &[libretto_model::base_libretto::CastMember],
    number_metas: &[align::NumberMeta],
    segments: Vec<libretto_model::base_libretto::Segment>,
) -> Result<BaseLibretto> {
    let mut libretto = BaseLibretto::new(metadata);
    libretto.cast = cast_members.to_vec();

    // Group segments back into their numbers
    let mut seg_iter = segments.into_iter();
    for meta in number_metas {
        let number_segments: Vec<_> = (&mut seg_iter).take(meta.segment_count).collect();
        libretto.numbers.push(MusicalNumber {
            id: meta.id.clone(),
            label: meta.label.clone(),
            number_type: meta.number_type.clone(),
            act: meta.act.clone(),
            scene: meta.scene.clone(),
            segments: number_segments,
        });
    }

    Ok(libretto)
}
