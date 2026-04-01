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
//! -
//!
//!
#![allow(dead_code, unused_variables, unused_mut)]

pub mod archiver;
pub mod builder;
pub mod header;

const RECORD_SIZE: usize = 512;

pub fn list_archive(
    file: &std::path::Path,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    todo!()
}

pub fn extract_archive(
    path: &std::path::Path,
    verbose: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    todo!()
}

pub fn create_archive(files: &[std::path::PathBuf]) -> Result<(), Box<dyn std::error::Error>> {
    todo!()
}
