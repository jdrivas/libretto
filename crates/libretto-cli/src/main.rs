use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "libretto")]
#[command(about = "Opera libretto acquisition, parsing, and validation tool")]
#[command(version = concat!(env!("CARGO_PKG_VERSION"), " (", env!("BUILD_HASH"), ")"))]
struct Cli {
    /// Log level: error, warn, info, debug, trace
    #[arg(long, global = true, default_value = "info", value_enum)]
    log_level: LogLevel,

    /// Use UTC timestamps instead of local time
    #[arg(long, global = true)]
    utc: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Clone, clap::ValueEnum)]
enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

#[derive(Subcommand)]
enum Commands {
    /// Acquire recording data: uses Claude to identify the recording, then optionally Discogs for track listing
    AcquireRecording {
        /// Opera identifier (e.g., "mozart/le-nozze-di-figaro")
        #[arg(short, long)]
        opera: String,

        /// Free-text query: "conductor, year, label, orchestra, with artist"
        #[arg(short, long)]
        query: String,

        /// Output directory for the timing JSON
        #[arg(short = 'O', long, default_value = ".")]
        output_dir: String,

        /// Anthropic API key (or set ANTHROPIC_API_KEY env var)
        #[arg(long, env = "ANTHROPIC_API_KEY")]
        api_key: String,

        /// Claude model to use
        #[arg(long, default_value = "claude-sonnet-4-20250514")]
        model: String,

        /// Discogs personal access token (or set DISCOGS_TOKEN env var). If omitted, only LLM search runs.
        #[arg(long, env = "DISCOGS_TOKEN")]
        discogs_token: Option<String>,
    },

    /// Acquire raw libretto text from online sources
    Acquire {
        /// Source site to fetch from
        #[arg(short, long, value_enum)]
        source: AcquireSource,

        /// Opera identifier (e.g., "mozart/le-nozze-di-figaro")
        #[arg(short, long)]
        opera: String,

        /// Languages: "it,en" (opera-arias, one per page), "en+it" (murashev, side-by-side), or "en"/"it" (single language)
        #[arg(short, long, default_value = "it,en")]
        lang: String,

        /// Output directory for raw text files
        #[arg(short = 'O', long, default_value = ".")]
        output_dir: String,
    },

    /// Parse raw libretto text into structured base libretto JSON
    Parse {
        /// Input directory containing raw text files (italian.txt, english.txt)
        #[arg(short, long)]
        input: String,

        /// Output file path for the base libretto JSON
        #[arg(short, long, default_value = "merged_score_libretto.json")]
        output: String,
    },

    /// Validate a base libretto or timing overlay file
    Validate {
        /// Path to the file to validate (.libretto.json or .timing.json)
        file: String,

        /// For timing overlays: path to the base libretto to check segment references against
        #[arg(short, long)]
        base: Option<String>,
    },

    /// Timing overlay tools: init, validate, merge
    Timing {
        #[command(subcommand)]
        action: TimingAction,
    },

    /// Score-derived annotation tools
    Annotate {
        #[command(subcommand)]
        action: AnnotateAction,
    },

    /// Generate structured data from score and libretto sources
    Generate {
        #[command(subcommand)]
        action: GenerateAction,
    },
}

#[derive(Subcommand)]
enum TimingAction {
    /// Generate a scaffold timing overlay from a base libretto
    Init {
        /// Path to the base libretto JSON
        #[arg(short, long)]
        base: String,

        /// Output path for the timing overlay JSON
        #[arg(short, long, default_value = "timing.overlay.json")]
        output: String,
    },

    /// Resolve track title anchors to segment IDs (populates start_segment_id)
    Resolve {
        /// Path to the base libretto JSON
        #[arg(short, long)]
        base: String,

        /// Path to the timing overlay JSON
        #[arg(short, long)]
        timing: String,

        /// Output path for the resolved timing overlay
        #[arg(short, long, default_value = "resolved.timing.json")]
        output: String,
    },

    /// Estimate segment timings from track durations and word counts
    Estimate {
        /// Path to the base libretto JSON
        #[arg(short, long)]
        base: String,

        /// Path to the timing overlay JSON (must have duration_seconds on tracks)
        #[arg(short, long)]
        timing: String,

        /// Output path for the updated timing overlay with estimated segment_times
        #[arg(short, long, default_value = "estimated.timing.json")]
        output: String,
    },

    /// Merge a base libretto + timing overlay into an interchange libretto
    Merge {
        /// Path to the base libretto JSON
        #[arg(short, long)]
        base: String,

        /// Path to the timing overlay JSON
        #[arg(short, long)]
        timing: String,

        /// Output path for the interchange libretto JSON
        #[arg(short, long, default_value = "timed.libretto.json")]
        output: String,
    },

    /// Merge a grouped score libretto with timing data into a track-centric output
    ScoreMerge {
        /// Path to the grouped score libretto JSON
        #[arg(short, long)]
        libretto: String,

        /// Path to the recording timing JSON
        #[arg(short, long)]
        timing: String,

        /// Output path for the merged JSON
        #[arg(short, long, default_value = "score_merged.json")]
        output: String,
    },

    /// Convert a timed score-merged JSON into interchange format for roon-rd
    ScoreInterchange {
        /// Path to the score-merged JSON (with timing)
        #[arg(short, long)]
        input: String,

        /// Output path for the interchange JSON
        #[arg(short, long, default_value = "interchange.json")]
        output: String,
    },
}

#[derive(Subcommand)]
enum AnnotateAction {
    /// Extract all pages from a score PDF as PNG images
    Ingest {
        /// Path to the vocal score PDF
        #[arg(short, long)]
        score: String,

        /// Output directory for page images
        #[arg(short, long)]
        output_dir: String,

        /// DPI for rendering (default: 150)
        #[arg(long, default_value = "150")]
        dpi: u32,
    },

    /// Build a score map: identify which pages belong to which musical numbers
    Map {
        /// Path to the base libretto JSON
        #[arg(short, long)]
        base: String,

        /// Directory containing ingested page images
        #[arg(short, long)]
        pages_dir: String,

        /// Output path for the score map JSON
        #[arg(short, long, default_value = "score_map.json")]
        output: String,

        /// Anthropic API key (or set ANTHROPIC_API_KEY env var)
        #[arg(long, env = "ANTHROPIC_API_KEY")]
        api_key: String,

        /// Claude model to use
        #[arg(long, default_value = "claude-sonnet-4-20250514")]
        model: String,

        /// Batch size: how many pages to send per API call
        #[arg(long, default_value = "20")]
        batch_size: usize,

        /// Path to existing page_descriptions.json — skip Phase 1 (LLM) and re-run Phase 2 (matching) only
        #[arg(long)]
        descriptions: Option<String>,
    },

    /// Generate annotations for numbers using a score map or manual page range
    Generate {
        /// Path to the base libretto JSON
        #[arg(short, long)]
        base: String,

        /// Directory containing ingested page images
        #[arg(short = 'd', long)]
        pages_dir: Option<String>,

        /// Path to the vocal score PDF (used if --pages-dir not provided)
        #[arg(short, long)]
        score: Option<String>,

        /// Score map JSON (annotates all mapped numbers)
        #[arg(short, long)]
        map: Option<String>,

        /// Musical number ID (for single-number mode)
        #[arg(short, long)]
        number: Option<String>,

        /// Page range for single-number mode (e.g., "220-232")
        #[arg(short, long)]
        pages: Option<String>,

        /// Output annotation file (created or updated)
        #[arg(short, long, default_value = "annotations.json")]
        output: String,

        /// Anthropic API key (or set ANTHROPIC_API_KEY env var)
        #[arg(long, env = "ANTHROPIC_API_KEY")]
        api_key: String,

        /// Claude model to use
        #[arg(long, default_value = "claude-sonnet-4-20250514")]
        model: String,
    },

    /// Apply a score annotation file to a base libretto
    Apply {
        /// Path to the base libretto JSON
        #[arg(short, long)]
        base: String,

        /// Path to the score annotation JSON
        #[arg(short, long)]
        annotations: String,

        /// Output path for the annotated base libretto
        #[arg(short, long, default_value = "annotated.base.libretto.json")]
        output: String,
    },

    /// Annotate segment groupings (simultaneous singing) using LLM + score images
    Groups {
        /// Path to merged_score_libretto.json
        #[arg(short, long)]
        input: String,

        /// Directory containing score page images (p001.png, p002.png, ...)
        #[arg(short = 'd', long)]
        pages_dir: String,

        /// Output path for the annotated JSON
        #[arg(short, long, default_value = "grouped_score_libretto.json")]
        output: String,

        /// Anthropic API key (or set ANTHROPIC_API_KEY env var)
        #[arg(long, env = "ANTHROPIC_API_KEY")]
        api_key: String,

        /// Claude model to use
        #[arg(long, default_value = "claude-sonnet-4-20250514")]
        model: String,
    },
}

#[derive(Subcommand)]
enum GenerateAction {
    /// Build a structured score.json from page_descriptions.json
    Score {
        /// Path to page_descriptions.json
        #[arg(short, long)]
        pages: String,

        /// Output path for the structured score JSON
        #[arg(short, long, default_value = "score.json")]
        output: String,
    },

    /// Restructure bilingual.json: extract front_matter (source, languages, cast) from rows
    Libretto {
        /// Path to bilingual.json
        #[arg(short, long)]
        bilingual: String,

        /// Output path for the restructured libretto JSON
        #[arg(short, long, default_value = "libretto.json")]
        output: String,
    },

    /// Merge score.json + libretto.json into merged_score_libretto.json
    Base {
        /// Path to score.json
        #[arg(short, long)]
        score: String,

        /// Path to libretto.json
        #[arg(short, long)]
        libretto: String,

        /// Output path for the merged base libretto JSON
        #[arg(short, long, default_value = "merged_score_libretto.json")]
        output: String,
    },
}

#[derive(Clone, clap::ValueEnum)]
enum AcquireSource {
    /// opera-arias.com (server-rendered, one page per language)
    OperaArias,
    /// murashev.com (server-rendered, side-by-side bilingual tables)
    Murashev,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Map log level, suppressing noisy HTML-parsing crates at debug/trace
    let level = match cli.log_level {
        LogLevel::Error => "error",
        LogLevel::Warn  => "warn",
        LogLevel::Info  => "info",
        LogLevel::Debug => "debug,selectors=warn,html5ever=warn",
        LogLevel::Trace => "trace,selectors=warn,html5ever=warn",
    };
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(level));

    // Timestamp format: 2026-02-14 19:44:09.123 -08:00
    let time_format = "%Y-%m-%d %H:%M:%S%.3f %:z";

    if cli.utc {
        tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .with_timer(tracing_subscriber::fmt::time::ChronoUtc::new(time_format.to_string()))
            .init();
    } else {
        tracing_subscriber::fmt()
            .with_env_filter(env_filter)
            .with_timer(tracing_subscriber::fmt::time::ChronoLocal::new(time_format.to_string()))
            .init();
    }

    match cli.command {
        Commands::AcquireRecording {
            opera,
            query,
            output_dir,
            api_key,
            model,
            discogs_token,
        } => {
            tracing::info!(opera = %opera, query = %query, "Acquiring recording data");

            // Phase 1: LLM search to identify the recording
            let llm_result = libretto_acquire::recording::search_with_llm(
                &opera, &query, &api_key, &model,
            ).await?;

            // Write LLM search result
            let dir = std::path::Path::new(&output_dir);
            std::fs::create_dir_all(dir)?;
            let llm_json = serde_json::to_string_pretty(&llm_result)?;
            let llm_path = dir.join("llm_search.json");
            std::fs::write(&llm_path, &llm_json)?;
            tracing::info!(path = %llm_path.display(), "Wrote LLM search result");

            // Phase 2: Wikipedia scraping for track listing + Discogs metadata
            let wiki_ok = libretto_acquire::recording::acquire_recording_from_wikipedia(
                &opera, &llm_result, &output_dir,
                discogs_token.as_deref(),
            ).await;

            match wiki_ok {
                Ok(()) => {
                    tracing::info!("Successfully acquired recording from Wikipedia");
                }
                Err(e) => {
                    tracing::warn!(error = %e, "Wikipedia scraping failed, trying Discogs fallback");

                    // Phase 3: Discogs fallback (if token provided)
                    if let Some(ref token) = discogs_token {
                        libretto_acquire::recording::acquire_recording(
                            &opera, &query, &output_dir, token,
                        ).await?;
                    } else {
                        anyhow::bail!(
                            "Wikipedia scraping failed ({e}) and no DISCOGS_TOKEN set for fallback"
                        );
                    }
                }
            }
        }
        Commands::Acquire {
            source,
            opera,
            lang,
            output_dir,
        } => {
            tracing::info!(opera = %opera, lang = %lang, "Acquiring libretto text");
            match source {
                AcquireSource::OperaArias => {
                    libretto_acquire::opera_arias::acquire(&opera, &lang, &output_dir).await?;
                }
                AcquireSource::Murashev => {
                    libretto_acquire::murashev::acquire(&opera, &lang, &output_dir).await?;
                }
            }
        }
        Commands::Parse { input, output } => {
            tracing::info!(input = %input, output = %output, "Parsing raw text");
            libretto_parse::parse(&input, &output)?;
        }
        Commands::Validate { file, base } => {
            tracing::info!(file = %file, "Validating");
            libretto_validate::validate(&file, base.as_deref())?;
        }
        Commands::Annotate { action } => match action {
            AnnotateAction::Ingest { score, output_dir, dpi } => {
                tracing::info!(score = %score, output_dir = %output_dir, dpi = dpi, "Ingesting score pages");
                let count = libretto_model::annotate::ingest_score(
                    std::path::Path::new(&score),
                    std::path::Path::new(&output_dir),
                    dpi,
                ).map_err(|e| anyhow::anyhow!("{e}"))?;
                tracing::info!(pages = count, path = %output_dir, "Score ingestion complete");
            }
            AnnotateAction::Map { base, pages_dir, output, api_key, model, batch_size, descriptions } => {
                tracing::info!(base = %base, pages_dir = %pages_dir, "Building score map (two-phase)");
                let base_contents = std::fs::read_to_string(&base)?;
                let base_libretto: libretto_model::BaseLibretto =
                    serde_json::from_str(&base_contents)?;

                let descs = if let Some(desc_path) = descriptions {
                    // Skip Phase 1: load existing page descriptions
                    tracing::info!(path = %desc_path, "Loading existing page descriptions (skipping Phase 1)");
                    let desc_contents = std::fs::read_to_string(&desc_path)?;
                    serde_json::from_str::<libretto_model::annotate::PageDescriptions>(&desc_contents)?
                } else {
                    // Phase 1: Have Claude describe each page (no base libretto reference)
                    let descs = libretto_model::annotate::generate::describe_pages(
                        std::path::Path::new(&pages_dir),
                        &api_key,
                        &model,
                        batch_size,
                    ).await.map_err(|e| anyhow::anyhow!("{e}"))?;

                    // Save intermediate page descriptions for debugging
                    let descs_path = std::path::Path::new(&output)
                        .with_file_name("page_descriptions.json");
                    let descs_json = serde_json::to_string_pretty(&descs)?;
                    std::fs::write(&descs_path, &descs_json)?;
                    tracing::info!(
                        pages = descs.pages.len(),
                        path = %descs_path.display(),
                        "Wrote page descriptions"
                    );
                    descs
                };

                // Phase 2: Match descriptions to base libretto numbers (pure Rust)
                let score_map = libretto_model::annotate::generate::match_pages_to_numbers(
                    &descs,
                    &base_libretto,
                );

                let json = serde_json::to_string_pretty(&score_map)?;
                std::fs::write(&output, &json)?;
                tracing::info!(
                    numbers = score_map.numbers.len(),
                    total_pages = score_map.total_pages,
                    path = %output,
                    "Wrote score map"
                );
            }
            AnnotateAction::Generate { base, pages_dir, score, map, number, pages, output, api_key, model } => {
                let base_contents = std::fs::read_to_string(&base)?;
                let base_libretto: libretto_model::BaseLibretto =
                    serde_json::from_str(&base_contents)?;

                let config = libretto_model::annotate::generate::GenerateConfig {
                    pages_dir: pages_dir.map(|s| s.into()),
                    score_path: score.clone().map(|s| s.into()),
                    api_key,
                    model,
                    temp_dir: std::env::temp_dir().join("libretto-annotate"),
                };

                // Determine what to annotate: score map (batch) or single number
                let numbers_to_annotate: Vec<(String, Vec<u32>)> = if let Some(map_path) = map {
                    // Batch mode: annotate all numbers from the score map
                    let map_contents = std::fs::read_to_string(&map_path)?;
                    let score_map: libretto_model::annotate::ScoreMap =
                        serde_json::from_str(&map_contents)?;
                    score_map.numbers.iter()
                        .map(|e| (e.number_id.clone(), e.pages.clone()))
                        .collect()
                } else if let (Some(number), Some(pages)) = (number, pages) {
                    // Single number mode
                    let page_nums: Vec<u32> = if pages.contains('-') {
                        let parts: Vec<&str> = pages.split('-').collect();
                        if parts.len() != 2 {
                            anyhow::bail!("Invalid page range: {}", pages);
                        }
                        let start: u32 = parts[0].trim().parse()?;
                        let end: u32 = parts[1].trim().parse()?;
                        (start..=end).collect()
                    } else {
                        pages.split(',').map(|s| s.trim().parse::<u32>()).collect::<Result<Vec<_>, _>>()?
                    };
                    vec![(number, page_nums)]
                } else {
                    anyhow::bail!("Provide either --map or both --number and --pages");
                };

                tracing::info!(numbers = numbers_to_annotate.len(), "Generating annotations");

                // Load or create annotation file
                let mut score_ann = if std::path::Path::new(&output).exists() {
                    let contents = std::fs::read_to_string(&output)?;
                    serde_json::from_str::<libretto_model::annotate::ScoreAnnotation>(&contents)?
                } else {
                    libretto_model::annotate::ScoreAnnotation {
                        source: libretto_model::annotate::AnnotationSource {
                            score_title: score.unwrap_or_default(),
                            score_id: None,
                        },
                        numbers: Vec::new(),
                    }
                };

                let total = numbers_to_annotate.len();
                for (i, (number_id, page_nums)) in numbers_to_annotate.iter().enumerate() {
                    tracing::info!(
                        number = %number_id,
                        pages = page_nums.len(),
                        progress = format!("{}/{}", i + 1, total),
                        "Annotating"
                    );

                    let number_ann = match libretto_model::annotate::generate::generate_number_annotation(
                        &config, &base_libretto, number_id, page_nums,
                    ).await {
                        Ok(ann) => ann,
                        Err(e) => {
                            tracing::error!(number = %number_id, error = %e, "Failed to annotate, skipping");
                            continue;
                        }
                    };

                    // Update or append
                    if let Some(existing) = score_ann.numbers.iter_mut().find(|n| n.number_id == *number_id) {
                        *existing = number_ann;
                    } else {
                        score_ann.numbers.push(number_ann);
                    }

                    // Write after each number so progress isn't lost
                    let json = serde_json::to_string_pretty(&score_ann)?;
                    std::fs::write(&output, &json)?;
                }

                tracing::info!(
                    total_numbers = score_ann.numbers.len(),
                    path = %output,
                    "Annotation generation complete"
                );
            }
            AnnotateAction::Groups { input, pages_dir, output, api_key, model } => {
                tracing::info!(input = %input, pages_dir = %pages_dir, output = %output, "Annotating segment groups");

                let config = libretto_acquire::grouping::GroupingConfig {
                    input_path: input.into(),
                    pages_dir: pages_dir.into(),
                    output_path: output.into(),
                    api_key,
                    model,
                };

                let summary = libretto_acquire::grouping::annotate_groups(&config).await?;

                tracing::info!(
                    total_numbers = summary.total_numbers,
                    with_segments = summary.numbers_with_segments,
                    processed = summary.numbers_processed,
                    with_groups = summary.numbers_with_groups,
                    total_groups = summary.total_groups,
                    grouped_segments = summary.total_grouped_segments,
                    errors = summary.errors.len(),
                    "Grouping annotation complete"
                );

                if !summary.errors.is_empty() {
                    for e in &summary.errors {
                        tracing::warn!("  {e}");
                    }
                }
            }
            AnnotateAction::Apply { base, annotations, output } => {
                tracing::info!(base = %base, annotations = %annotations, output = %output, "Applying score annotations");
                let base_contents = std::fs::read_to_string(&base)?;
                let mut base_libretto: libretto_model::BaseLibretto =
                    serde_json::from_str(&base_contents)?;
                let ann_contents = std::fs::read_to_string(&annotations)?;
                let score_ann: libretto_model::annotate::ScoreAnnotation =
                    serde_json::from_str(&ann_contents)?;

                let result = libretto_model::annotate::apply(&mut base_libretto, &score_ann);
                for w in &result.warnings {
                    tracing::warn!("{w}");
                }

                let json = serde_json::to_string_pretty(&base_libretto)?;
                std::fs::write(&output, &json)?;
                tracing::info!(
                    tempo_markings = result.tempo_markings_set,
                    number_types = result.number_types_set,
                    groups = result.groups_applied,
                    path = %output,
                    "Applied annotations"
                );
            }
        },
        Commands::Timing { action } => match action {
            TimingAction::Init { base, output } => {
                tracing::info!(base = %base, output = %output, "Generating scaffold timing overlay");
                let base_contents = std::fs::read_to_string(&base)?;
                let base_libretto: libretto_model::BaseLibretto =
                    serde_json::from_str(&base_contents)?;
                let overlay = libretto_model::merge::scaffold_overlay(&base_libretto, &base);
                let json = serde_json::to_string_pretty(&overlay)?;
                std::fs::write(&output, &json)?;
                let seg_count: usize = overlay.track_timings.iter()
                    .map(|t| t.segment_times.len())
                    .sum();
                tracing::info!(
                    tracks = overlay.track_timings.len(),
                    segments = seg_count,
                    path = %output,
                    "Wrote scaffold timing overlay"
                );
            }
            TimingAction::Resolve { base, timing, output } => {
                tracing::info!(base = %base, timing = %timing, output = %output, "Resolving track anchors");
                let base_contents = std::fs::read_to_string(&base)?;
                let base_libretto: libretto_model::BaseLibretto =
                    serde_json::from_str(&base_contents)?;
                let overlay_contents = std::fs::read_to_string(&timing)?;
                let overlay: libretto_model::TimingOverlay =
                    serde_json::from_str(&overlay_contents)?;

                let result = libretto_model::resolve::resolve_anchors(&base_libretto, &overlay);
                for w in &result.warnings {
                    tracing::warn!("{w}");
                }
                let mut resolved = 0;
                let mut unresolved = 0;
                for res in &result.resolutions {
                    let disc = res.disc_number.unwrap_or(0);
                    let track = res.track_number.unwrap_or(0);
                    match &res.resolved_segment_id {
                        Some(seg_id) => {
                            let method = res.match_method.as_ref()
                                .map(|m| format!("{:?}", m))
                                .unwrap_or_default();
                            tracing::info!(
                                disc = disc,
                                track = track,
                                segment = %seg_id,
                                method = %method,
                                anchors = ?res.anchors,
                                "Resolved"
                            );
                            resolved += 1;
                        }
                        None => {
                            tracing::warn!(
                                disc = disc,
                                track = track,
                                anchors = ?res.anchors,
                                "Unresolved"
                            );
                            unresolved += 1;
                        }
                    }
                }
                let json = serde_json::to_string_pretty(&result.overlay)?;
                std::fs::write(&output, &json)?;
                tracing::info!(
                    resolved = resolved,
                    unresolved = unresolved,
                    total = result.resolutions.len(),
                    path = %output,
                    "Wrote resolved timing overlay"
                );
            }
            TimingAction::Estimate { base, timing, output } => {
                tracing::info!(base = %base, timing = %timing, output = %output, "Estimating segment timings");
                let base_contents = std::fs::read_to_string(&base)?;
                let base_libretto: libretto_model::BaseLibretto =
                    serde_json::from_str(&base_contents)?;
                let overlay_contents = std::fs::read_to_string(&timing)?;
                let overlay: libretto_model::TimingOverlay =
                    serde_json::from_str(&overlay_contents)?;

                let result = libretto_model::estimate::estimate_timings(&base_libretto, &overlay);
                for w in &result.warnings {
                    tracing::warn!("{w}");
                }
                for stat in &result.stats {
                    tracing::info!(
                        track = %stat.track_title,
                        disc = ?stat.disc_number,
                        num = ?stat.track_number,
                        duration = stat.duration,
                        segments = stat.segments_estimated,
                        word_weight = format!("{:.1}", stat.total_word_weight),
                        "Estimated"
                    );
                }
                let total_segs: usize = result.stats.iter().map(|s| s.segments_estimated).sum();
                let json = serde_json::to_string_pretty(&result.overlay)?;
                std::fs::write(&output, &json)?;
                tracing::info!(
                    segments = total_segs,
                    tracks = result.stats.len(),
                    path = %output,
                    "Wrote estimated timing overlay"
                );
            }
            TimingAction::Merge { base, timing, output } => {
                tracing::info!(base = %base, timing = %timing, output = %output, "Merging");
                let base_contents = std::fs::read_to_string(&base)?;
                let base_libretto: libretto_model::BaseLibretto =
                    serde_json::from_str(&base_contents)?;
                let overlay_contents = std::fs::read_to_string(&timing)?;
                let overlay: libretto_model::TimingOverlay =
                    serde_json::from_str(&overlay_contents)?;

                // Validate before merging
                let errors = libretto_validate::validate_timing_overlay(&overlay, &base_libretto)?;
                if !errors.is_empty() {
                    for e in &errors {
                        tracing::error!("{e}");
                    }
                    anyhow::bail!("{} validation errors — fix before merging", errors.len());
                }

                let result = libretto_model::merge::merge(&base_libretto, &overlay);
                for w in &result.warnings {
                    tracing::warn!("{w}");
                }
                let json = serde_json::to_string_pretty(&result.libretto)?;
                std::fs::write(&output, &json)?;
                tracing::info!(
                    tracks = result.stats.tracks,
                    segments = result.stats.merged_segments,
                    path = %output,
                    "Wrote interchange libretto"
                );
            }
            TimingAction::ScoreInterchange { input, output } => {
                tracing::info!(input = %input, output = %output, "Score to interchange");
                let contents = std::fs::read_to_string(&input)?;
                let merged: libretto_model::score_merge::ScoreMergedOutput =
                    serde_json::from_str(&contents)?;

                let interchange = libretto_model::score_merge::score_merged_to_interchange(&merged);

                let seg_count: usize = interchange.tracks.iter().map(|t| t.segments.len()).sum();
                let json = serde_json::to_string_pretty(&interchange)?;
                std::fs::write(&output, &json)?;
                tracing::info!(
                    tracks = interchange.tracks.len(),
                    segments = seg_count,
                    path = %output,
                    "Wrote interchange libretto"
                );
            }
            TimingAction::ScoreMerge { libretto, timing, output } => {
                tracing::info!(libretto = %libretto, timing = %timing, output = %output, "Score merge");
                let libretto_contents = std::fs::read_to_string(&libretto)?;
                let grouped: libretto_model::score_merge::GroupedScoreLibretto =
                    serde_json::from_str(&libretto_contents)?;
                let timing_contents = std::fs::read_to_string(&timing)?;
                let overlay: libretto_model::TimingOverlay =
                    serde_json::from_str(&timing_contents)?;

                // Extract discogs metadata (not part of TimingOverlay struct)
                let timing_value: serde_json::Value =
                    serde_json::from_str(&timing_contents)?;
                let discogs = timing_value.get("discogs").cloned();

                let result = libretto_model::score_merge::score_merge(
                    &grouped, &overlay, discogs,
                );

                for w in &result.warnings {
                    tracing::warn!("{w}");
                }
                for b in &result.boundaries {
                    tracing::info!(
                        disc = ?b.disc_number,
                        track = ?b.track_number,
                        score_number = b.score_number_id,
                        method = %b.method,
                        "Boundary"
                    );
                }

                // Estimate segment timings
                let mut merged_output = result.output;
                let (est_stats, est_warnings) =
                    libretto_model::score_merge::estimate_score_merge_timings(&mut merged_output);
                for w in &est_warnings {
                    tracing::warn!("{w}");
                }

                let json = serde_json::to_string_pretty(&merged_output)?;
                std::fs::write(&output, &json)?;
                tracing::info!(
                    matched_tracks = result.stats.matched_tracks,
                    total_tracks = result.stats.total_tracks,
                    assigned = result.stats.assigned_score_numbers,
                    total_numbers = result.stats.total_score_numbers,
                    segments = result.stats.total_segments,
                    tracks_timed = est_stats.tracks_estimated,
                    segments_timed = est_stats.segments_estimated,
                    path = %output,
                    "Wrote score-merged output with timing"
                );
            }
        },
        Commands::Generate { action } => match action {
            GenerateAction::Score { pages, output } => {
                tracing::info!(pages = %pages, output = %output, "Building structured score");
                let contents = std::fs::read_to_string(&pages)?;
                let page_descs: libretto_model::annotate::PageDescriptions =
                    serde_json::from_str(&contents)?;

                let score = libretto_model::score::build_score(&page_descs);

                let json = serde_json::to_string_pretty(&score)?;
                std::fs::write(&output, &json)?;
                tracing::info!(
                    numbers = score.numbers.len(),
                    total_pages = score.front_matter.total_pages,
                    path = %output,
                    "Wrote structured score"
                );
            }
            GenerateAction::Libretto { bilingual, output } => {
                tracing::info!(bilingual = %bilingual, output = %output, "Building structured libretto");
                let contents = std::fs::read_to_string(&bilingual)?;
                let raw: libretto_model::bilingual::RawBilingual =
                    serde_json::from_str(&contents)?;

                let libretto = libretto_model::bilingual::build_libretto(&raw);

                let num_numbers = libretto.numbers.len();
                let total_segments: usize = libretto.numbers.iter()
                    .map(|n| n.segments.len()).sum();

                let json = serde_json::to_string_pretty(&libretto)?;
                std::fs::write(&output, &json)?;
                tracing::info!(
                    numbers = num_numbers,
                    segments = total_segments,
                    path = %output,
                    "Wrote structured libretto"
                );
            }
            GenerateAction::Base { score, libretto, output } => {
                tracing::info!(score = %score, libretto = %libretto, output = %output, "Building base libretto");
                let score_contents = std::fs::read_to_string(&score)?;
                let score_data: libretto_model::score::Score =
                    serde_json::from_str(&score_contents)?;

                let lib_contents = std::fs::read_to_string(&libretto)?;
                let lib_data: libretto_model::bilingual::Libretto =
                    serde_json::from_str(&lib_contents)?;

                let result = libretto_model::build_base::build_base(&score_data, &lib_data);

                for w in &result.warnings {
                    tracing::warn!("{w}");
                }

                let total_segments: usize = result.merged.numbers.iter()
                    .map(|n| n.segments.len()).sum();
                let matched_numbers = result.merged.numbers.iter()
                    .filter(|n| !n.segments.is_empty()).count();

                let json = serde_json::to_string_pretty(&result.merged)?;
                std::fs::write(&output, &json)?;
                tracing::info!(
                    score_numbers = result.merged.numbers.len(),
                    matched = matched_numbers,
                    segments = total_segments,
                    warnings = result.warnings.len(),
                    path = %output,
                    "Wrote base libretto"
                );
            }
        },
    }

    Ok(())
}
