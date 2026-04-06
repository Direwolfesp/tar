//! Basic utilities to efficiently copying bytes between files

use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    path::Path,
};

/// Naively copy a range of bytes from one file to another
///
/// This function is mainly implemented as a fallback for [clone_or_copy_file_range].
///
/// Returns `Ok(copied_byte_count)` on success.
pub fn copy_file_range<P: AsRef<Path>>(
    src: P,
    src_offset: u64,
    src_length: u64,
    dest: P,
    dest_offset: u64,
) -> io::Result<u64> {
    let mut src_file = File::open(src)?;
    src_file.seek(SeekFrom::Start(src_offset))?;
    let mut src_file = src_file.take(src_length);

    let mut dest_file = File::options().write(true).create(true).open(dest)?;
    dest_file.seek(SeekFrom::Start(dest_offset))?;

    io::copy(&mut src_file, &mut dest_file)
}
