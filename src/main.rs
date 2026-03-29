use clap::Parser;
use std::{
    fs::File,
    io::{BufRead, BufReader},
};
use tar;

const ABOUT: &'static str = r"
    Implements some basic tar archive functionality but lacks convenient utilities like decompressing gzipped tarballs.
";

/// Simple tar cli program
#[derive(Parser, Debug)]
#[command(version, about, long_about = ABOUT)]
struct Args {
    /// Extract files from archive
    #[arg(short = 'x', long, action)]
    extract: bool,

    /// List the contents of an archive
    #[arg(short = 't', long, action)]
    list: bool,

    /// Create a new archive
    #[arg(short = 'c', long, action)]
    create: bool,

    /// Use archive file
    #[arg(short = 'f', long, required = false)]
    file: Option<String>,

    /// files
    files: Vec<String>,
}

trait DefaultToStdin {
    fn open(&self) -> Box<dyn BufRead>;
}

impl DefaultToStdin for Option<String> {
    fn open(&self) -> Box<dyn BufRead> {
        match self {
            None => Box::new(BufReader::new(std::io::stdin())),
            Some(filename) => match filename.as_str() {
                "-" => Box::new(BufReader::new(std::io::stdin())),
                _ => {
                    let file = File::open(filename).expect("Cannot open source file");
                    Box::new(BufReader::new(file))
                }
            },
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    if args.list {
        let archive = args.file.open();
        tar::list_archive(archive)?;
    }

    Ok(())
}
