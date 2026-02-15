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
    }

    Ok(())
}
