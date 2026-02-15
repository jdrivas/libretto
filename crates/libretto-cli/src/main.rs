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
        #[arg(short, long, default_value = "base.libretto.json")]
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
        },
    }

    Ok(())
}
