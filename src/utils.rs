/// Convert the ASCII bytes representing an octal number
/// into a valid integer, stripping necessary
/// NULL bytes at the end if necesary used in Tar fields.
///
/// Returns `None` if the slice is all zeroes (field is unused)
/// or an error ocurred parsing the octal.
pub fn get_number<T: num_traits::Num>(bytes: &[u8]) -> Option<T> {
    let bytes = rstrip(bytes)?;
    let v = bytes_to_str(bytes);
    T::from_str_radix(v, 8).ok()
}

/// Convert the bytes into a valid String, stripping necessary
/// NULL bytes at the end present in Tar fields
///
/// Returns `None` if the slice is all zeroes (field is unused)
/// or has an invalid ASCII string.
pub fn get_string(bytes: &[u8]) -> Option<String> {
    let bytes = rstrip(bytes)?;
    let s = bytes_to_str(bytes);
    Some(String::from(s))
}

/// Helper to convert bytes into valid str slices.
///
/// Panics.
fn bytes_to_str(bytes: &[u8]) -> &str {
    str::from_utf8(bytes)
        .expect("TODO: Error handling from invalid ascii characters. (Or corrupted header)")
}

/// Helper that removes all rightmost NULL bytes
/// from the slice. Returs `None` if the resulting
/// slice is of size 0, which means the field is unnused
///
/// Panics
///
/// ### Example:
/// - `[10, 32, 82, 02, 00, 00, 00] => Some([10, 32, 82, 02])`
/// - `[00, 00, 00, 00] => None`
fn rstrip(bytes: &[u8]) -> Option<&[u8]> {
    let stripped = bytes
        .split(|b| *b == 0)
        .next()
        .expect("TODO: handle split failure");

    if stripped.is_empty() {
        None
    } else {
        Some(stripped)
    }
}

/// Basic I/O related utilities
pub mod io {
    use crate::RECORD_SIZE;
    use std::{
        fs::File,
        io::{self, Read, Seek, SeekFrom},
        path::Path,
    };

    /// Aligns the file pointer forward to a record boundary
    pub fn align_forward(file: &mut File) {
        let pos = file.stream_position().unwrap();
        let rem = pos as usize % RECORD_SIZE;

        // only align forward if we are not in a position divisible by
        // RECORD_SIZE
        if rem != 0 {
            let align_forward = RECORD_SIZE - rem;
            file.seek_relative(align_forward as i64)
                .expect("align forward failed");
        }
    }

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
}
