use clap::Parser;
use tar;

const ABOUT: &str = "A minimal tar implementation made in Rust for learning purposes.";

/// Simple tar cli program
enum Commands {
    Extract,
    List,
    Create,
}

struct Args {
    /// Use archive file
    file: Option<String>,

    /// Filter the archive through gzip. (Requires 'gzip' in '$PATH')
    gzip: bool,

    /// More detailed output
    verbose: bool,

    /// files
    files: Vec<String>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    Ok(())
}
