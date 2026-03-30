//!
//! ## Basic tar format key notes
//! - All file objects are concatenated sequentaly through the tarball.
//! - Each file is preceded by a 512B header record.
//! - The end of a file data section must be rounded up to the neares 512B record.
//! - Padding is usually zeroed up.
//! - End of archive is marked by at least two zeroed records.
//!
//!
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
//!
//! ## Header parsing notes
//! - The information is encoded in ASCII.
//! - When a field is unused is filled with NULL bytes.
//! - Numeric fields are octal using ASCII digits with leading zeroes.
//! -
//!
//!
#![allow(dead_code, unused_variables, unused_mut)]
use std::{
    error, fs,
    io::{self, BufRead},
    iter::Iterator,
    path::PathBuf,
    time,
};

const RECORD_SIZE: i32 = 512;
const TERMINATOR: &str = "0\0";

pub fn list_archive(file: impl BufRead, verbose: bool) -> Result<(), Box<dyn error::Error>> {
    let tar = TarReader::new(file);
    tar.for_each(|h| h.unwrap().print_meta(verbose));

    Ok(())
}

pub fn extract_archive(file: impl BufRead, verbose: bool) -> Result<(), Box<dyn error::Error>> {
    todo!()
}

pub fn create_archive(files: &[PathBuf]) -> Result<(), Box<dyn error::Error>> {
    todo!()
}

// NOTE: Is using a generic stream (non seekable) reader appropiate? It would
// make sense to directly work on the file and make use of positional reading
// for jumping through the file. However we need to also support reading from
// stdin and pipes which are not seekable.
struct TarReader<R: BufRead> {
    file: R,
    pos: usize,
}

impl<R: BufRead> TarReader<R> {
    fn new(r: R) -> Self {
        Self { file: r, pos: 0 }
    }

    fn align_forward(&mut self) {
        todo!()
    }
}

impl<R: BufRead> Iterator for TarReader<R> {
    type Item = io::Result<Header>;

    fn next(&mut self) -> Option<Self::Item> {
        todo!()
    }
}

/// Generic tar header structure that tries to comply with both legacy and modern
/// POSIX format record.
struct Header {
    /// File path and name
    path: PathBuf,

    /// File mode (octal)
    mode: fs::Permissions,

    /// Owner's numeric user ID (octal)
    uid: u32,

    /// Group's numeric user ID (octal)
    gid: u32,

    /// File size in bytes (octal)
    file_size: u64,

    /// Last modification time in numeric Unix time format (octal)
    mtime: time::Instant,

    /// Checksum for header record
    checksum: u64,

    /// File type
    type_flag: PosixTypeFlag,

    /// Name of linked file.
    ///
    /// If several files with the same name appear in a tar archive, only the first one is
    /// archived as a normal file; the rest are archived as hard links, with the
    /// "name of linked file" field set to the first one's name. On extraction,
    /// such hard links should be recreated in the file system.
    linked_file: Option<String>,

    /// Only present for post 1988 POSIX IEEE standard archives. Which is almost the case
    /// nowadays. Must check for the presence of "ustar\0" at offset 257
    posix_extension: Option<PosixHeader>,
}

impl Header {
    /// The checksum is calculated by taking the sum of the {un}signed byte
    /// values of the header record with the eight checksum bytes taken to
    /// be ASCII spaces (decimal value 32). It is stored as a six digit octal
    /// number with leading zeroes followed by a NUL and then a space.
    fn verify_checksum(&self) -> bool {
        todo!()
    }

    fn print_meta(&self, verbose: bool) {
        todo!()
    }
}

struct PosixHeader {
    /// Ustar version, "00"
    version: u16,

    /// Owner user name
    owner: String,

    /// Owner group name
    group: String,

    /// Device major number
    device_major: Option<u32>,

    /// Device minor number
    device_minor: Option<u32>,

    /// Filename prefix
    filename_prefix: Option<String>,
}

/// Its also compatible with the old pre-posix file flag
#[repr(u8)]
enum PosixTypeFlag {
    NormalFile = b'0',
    HardLink = b'1',
    SymLink = b'2',
    CharDevice = b'3',
    BlockDevice = b'4',
    Directory = b'5',
    Fifo = b'6',
    ContiguousFile = b'7', // Same as normal file
    GlobalExtHeader = b'g',
    ExtHeader = b'x',
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn foo() {
        todo!();
    }
}
