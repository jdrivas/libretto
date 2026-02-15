use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "libretto")]
#[command(about = "Opera libretto acquisition, parsing, and validation tool")]
#[command(version)]
struct Cli {
    /// Enable verbose logging
    #[arg(short, long, global = true)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
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

    let default_filter = if cli.verbose {
        "debug,selectors=warn,html5ever=warn"
    } else {
        "info"
    };
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(default_filter)),
        )
        .init();

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
