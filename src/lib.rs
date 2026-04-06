//!
//! ## Basic tar format key notes
//! - All file objects are concatenated sequentaly through the tarball.
//! - Each file is preceded by a 512B header record.
//! - The end of a file data section must be rounded up to the neares 512B record.
//! - Padding is usually zeroed up.
//! - End of archive is marked by at least two zeroed records.
//!
//!
//! ```text
//!                                               padding to round up to 512
//!                                                           |
//!                                                           |
//! Offset 0       512                     k*512   (k+1)*512  v
//!        +--------+------------------------+--------+--------+--------+--------+
//!        |        |                        |        |    |...|........|........|
//! Memory | Header |  Data                  | Header |Data|...|........|........| End
//!        |        |                        |        |    |...|........|........|
//!        +--------+------------------------+--------+----+---+--------+--------+
//!                                                            \_________________/
//!                                                                     |
//!                                                                     |
//!                                                           two zero-filled records
//! ```
//!
//! ## Header parsing notes
//! - The information is encoded in ASCII.
//! - When a field is unused is filled with NULL bytes.
//! - Numeric fields are octal using ASCII digits with leading zeroes.
//!
#![allow(dead_code, unused_variables, unused_mut)]

mod archiver;
mod builder;
mod header;
mod io;

use std::path::{Path, PathBuf};

// re-export names
pub use archiver::Archiver;
pub use header::Header;

const RECORD_SIZE: usize = 512;

pub fn list_archive(file: &Path, verbose: bool) -> Result<(), std::io::Error> {
    if !file.exists() {
        Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            format!("Cannot open archive '{}'", file.to_string_lossy()),
        ))
    } else {
        let tar = Archiver::parse(file);
        tar.print_files(verbose);
        Ok(())
    }
}

pub fn extract_archive(file: &Path, dest: &Path, verbose: bool) -> Result<(), std::io::Error> {
    let mut tar = Archiver::parse(file);
    tar.extract_to_dir(dest)?;
    if verbose {
        eprintln!("Extracted contents to {}", dest.display());
    }
    Ok(())
}

pub fn create_archive(files: &[PathBuf]) -> Result<(), std::io::Error> {
    todo!()
}
