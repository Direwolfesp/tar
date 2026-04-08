use clap::{Parser, Subcommand, ValueHint};
use log::LevelFilter;
use std::{io::Write, path::PathBuf};

const ABOUT: &str = "A minimal tar implementation made in Rust for learning purposes.";

#[derive(Subcommand)]
enum Command {
    /// Extract an archive file
    #[clap(visible_alias = "e")]
    Extract {
        /// Output directory.
        #[arg(short, long, default_value = ".", value_hint = ValueHint::DirPath)]
        output: PathBuf,

        /// Archive to extract
        archive: PathBuf,
    },

    /// Lists files of an archive
    #[clap(visible_aliases = ["l", "ls"])]
    List {
        /// Shows extra information
        #[arg(short, long)]
        long: bool,

        /// Use archive file
        archive: PathBuf,
    },

    /// Creates a new tar archive
    #[clap(visible_alias = "c")]
    Create {
        /// Name of the archive
        name: PathBuf,

        /// files to be included in the archive
        files: Vec<PathBuf>,
    },
}

/// Simple tar cli program
#[derive(Parser)]
#[command(version, about = ABOUT)]
struct Args {
    /// Action to perform
    #[command(subcommand)]
    action: Command,

    /// Reduces terminal output
    #[arg(short, long)]
    quiet: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_logging();
    let args = Args::parse();

    match args.action {
        Command::List { archive, long } => tar::list_archive(archive.as_path(), long)?,
        Command::Extract { archive, output } => {
            tar::extract_archive(archive.as_path(), output.as_path(), !args.quiet)?
        }
        Command::Create { files: _, name: _ } => {
            todo!("implemnent creating archive")
        }
    }

    Ok(())
}

fn init_logging() {
    let mut builder = env_logger::Builder::new();
    builder
        .format(|buf, record| {
            let warn_style = buf.default_level_style(record.level());
            writeln!(
                buf,
                "{warn_style}[{}]{warn_style:#}: {}",
                record.level(),
                record.args()
            )
        })
        .filter_level(LevelFilter::Info)
        .init();
}
