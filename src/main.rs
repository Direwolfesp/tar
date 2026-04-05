use std::path::PathBuf;

use clap::{Parser, Subcommand};

const ABOUT: &str = "A minimal tar implementation made in Rust for learning purposes.";

#[derive(Subcommand)]
enum Command {
    /// Extract an archive file
    Extract {
        /// Output directory.
        #[arg(short, long, default_value = ".")]
        output: Option<PathBuf>,

        /// Archive to extract
        archive: PathBuf,
    },
    /// Lists files of an archive
    List {
        /// Verbose output
        #[arg(short, long)]
        verbose: bool,

        /// Use archive file
        archive: PathBuf,
    },
    /// Creates a new tar archive
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
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    match args.action {
        Command::List { archive, verbose } => tar::list_archive(archive.as_path(), verbose)?,
        Command::Extract {
            archive: _,
            output: _,
        } => {
            todo!("implement extracting")
        }
        Command::Create { files: _, name: _ } => {
            todo!("implemnent creating archive")
        }
    }

    Ok(())
}
