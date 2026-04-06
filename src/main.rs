use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueHint};

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

    /// Verbose output
    #[arg(short, long)]
    verbose: bool,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    match args.action {
        Command::List { archive } => tar::list_archive(archive.as_path(), args.verbose)?,
        Command::Extract { archive, output } => {
            tar::extract_archive(archive.as_path(), output.as_path(), args.verbose)?
        }
        Command::Create { files: _, name: _ } => {
            todo!("implemnent creating archive")
        }
    }

    Ok(())
}
