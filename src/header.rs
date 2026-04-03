use std::{
    fs,
    num::ParseIntError,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

use crate::RECORD_SIZE;
use chrono::{DateTime, TimeZone, Utc};
use thiserror::Error;
use utils::FieldRange;

/// Generic tar header structure that tries to comply with both legacy and modern
/// POSIX format record.
#[derive(Debug)]
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
    mtime: DateTime<Utc>,

    /// Checksum for header record
    checksum: u64,

    /// File type
    type_flag: TypeFlag,

    /// Name of linked file.
    ///
    /// If several files with the same name appear in a tar archive, only the first one is
    /// archived as a normal file; the rest are archived as hard links, with the
    /// "name of linked file" field set to the first one's name. On extraction,
    /// such hard links should be recreated in the file system.
    linked_file: Option<String>,

    /// Only present for post 1988 POSIX IEEE standard archives. Which is almost the case
    /// nowadays. Must check for the presence of "ustar\0" at offset 257
    posix_header: Option<PosixHeader>,
}

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("Failed to parse octal ascii number: {0}")]
    InvalidNumber(#[from] ParseIntError),

    #[error("File object contains and invalid type flag")]
    BadFileTypeFlag(#[from] TypeFlagError),

    #[error("Missing primary field: {0}")]
    MissingField(String),

    #[error("Failed to verify header, is the file corrupted?")]
    ChecksumFailed,
}

mod utils {
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

        if stripped.len() == 0 {
            None
        } else {
            Some(stripped)
        }
    }

    /// Helper enum to convert Tar header fields into valid
    /// ranges inside a 512B header. Ex: `&raw_bytes[FieldRange::Path]`
    pub enum FieldRange {
        Path,
        Mode,
        Uid,
        Gid,
        FileSize,
        Mtime,
        Checksum,
        Typeflag,
        Linked,
        Ustar,
        Version,
        Owner,
        Group,
        Major,
        Minor,
        FilenamePrefix,
    }

    impl std::ops::Index<FieldRange> for [u8] {
        type Output = [u8];
        fn index(&self, index: FieldRange) -> &Self::Output {
            match index {
                FieldRange::Path => &self[0..100],
                FieldRange::Mode => &self[100..108],
                FieldRange::Uid => &self[108..116],
                FieldRange::Gid => &self[116..124],
                FieldRange::FileSize => &self[124..136],
                FieldRange::Mtime => &self[136..148],
                FieldRange::Checksum => &self[148..156],
                FieldRange::Typeflag => &self[156..157],
                FieldRange::Linked => &self[157..257],
                FieldRange::Ustar => &self[257..263],
                FieldRange::Version => &self[263..265],
                FieldRange::Owner => &self[265..297],
                FieldRange::Group => &self[297..329],
                FieldRange::Major => &self[329..337],
                FieldRange::Minor => &self[337..345],
                FieldRange::FilenamePrefix => &self[345..500],
            }
        }
    }
}

impl Header {
    pub fn parse(bytes: &[u8; RECORD_SIZE]) -> Result<Self, ParseError> {
        // PATH
        let path_string = utils::get_string(&bytes[FieldRange::Path])
            .ok_or(ParseError::MissingField("path".into()))?;
        let path = PathBuf::from(path_string);

        // MODE
        let mode_raw = utils::get_number(&bytes[FieldRange::Mode])
            .ok_or(ParseError::MissingField("mode".into()))?;
        let mode = fs::Permissions::from_mode(mode_raw);

        // UID
        let uid = utils::get_number(&bytes[FieldRange::Uid])
            .ok_or(ParseError::MissingField("uid".into()))?;

        // GID
        let gid = utils::get_number(&bytes[FieldRange::Gid])
            .ok_or(ParseError::MissingField("gid".into()))?;

        // FILE SIZE
        let file_size = utils::get_number(&bytes[FieldRange::FileSize])
            .ok_or(ParseError::MissingField("file size".into()))?;

        // MTIME
        let mtime_seconds = utils::get_number(&bytes[FieldRange::Mtime])
            .ok_or(ParseError::MissingField("mtime".into()))?;
        let mtime = Utc
            .timestamp_opt(mtime_seconds, 0)
            .single()
            .expect("Invalid mtime unix time");

        // CHECKSUM
        let checksum: u64 = utils::get_number(&bytes[FieldRange::Checksum])
            .ok_or(ParseError::MissingField("checksum".into()))?;

        // TYPE FLAG
        let flag_byte = &bytes[FieldRange::Typeflag];
        let type_flag = TypeFlag::try_new(flag_byte[0])?;

        // LINKED FILE
        let linked_file: Option<String> = utils::get_string(&bytes[FieldRange::Linked]);

        // POSIX HEADER (optional, common case)
        let posix_header = PosixHeader::from_record(bytes)?;

        let header = Header {
            path,
            mode,
            uid,
            gid,
            file_size,
            mtime,
            checksum,
            type_flag,
            linked_file,
            posix_header,
        };

        if header.verify_checksum(bytes) {
            Ok(header)
        } else {
            Err(ParseError::ChecksumFailed)
        }
    }

    /// get file owner user name
    pub fn owner(&self) -> Option<&str> {
        if let Some(posix) = &self.posix_header {
            posix.owner.as_deref()
        } else {
            return None;
        }
    }

    /// get file owner group name
    pub fn group(&self) -> Option<&str> {
        if let Some(posix) = &self.posix_header {
            posix.group.as_deref()
        } else {
            return None;
        }
    }

    /// get file path
    pub fn filename(&self) -> &Path {
        // TODO: handle symlinks maybe
        self.path.as_path()
    }

    /// get permissions mode
    pub fn mode(&self) -> u32 {
        self.mode.mode()
    }

    /// Returs a bool indicating if the checksum is valid.
    ///
    /// To compute the checksum:
    /// - set the checksum field to all spaces,
    /// - then sum all bytes in the header using unsigned arithmetic.
    ///
    /// Note that many early implementations of tar used signed arithmetic
    /// for the checksum field, which can cause interoperability problems when
    /// transferring archives between systems. Modern robust readers compute the
    /// checksum both ways and accept the header if either computation matches.
    fn verify_checksum(&self, raw_header: &[u8; RECORD_SIZE]) -> bool {
        let mut raw_header: [u8; RECORD_SIZE] = *raw_header;
        let checksum_bytes = &mut raw_header[148..156];
        for b in checksum_bytes {
            *b = b' ';
        }

        let checksum = self.checksum;
        let sum_unsigned: u64 = raw_header.iter().map(|x| *x as u64).sum();
        let sum_signed: i64 = raw_header.iter().map(|x| *x as i64).sum();

        if checksum as i64 == sum_signed || checksum == sum_unsigned {
            true
        } else {
            false
        }
    }

    // TODO: more complete implementation with more context
    fn print_meta(&self, verbose: bool) {
        println!("{}", self.filename().display())
    }
}

#[derive(Debug)]
struct PosixHeader {
    /// Ustar version, "00"
    version: u16,

    /// Owner user name
    owner: Option<String>,

    /// Owner group name
    group: Option<String>,

    /// Device major number
    device_major: Option<u32>,

    /// Device minor number
    device_minor: Option<u32>,

    /// Filename prefix
    filename_prefix: Option<String>,
}

impl PosixHeader {
    pub fn from_record(bytes: &[u8; RECORD_SIZE]) -> Result<Option<Self>, ParseError> {
        // Here I wont distinguish between POSIX ustar and
        // GNU Tar, and just accept any extented header.
        // Im just making it work with regular GNU tar utility.
        let ustar_magic = &bytes[FieldRange::Ustar];

        if ustar_magic != b"ustar " && ustar_magic != b"ustar\0" {
            return Ok(None);
        }

        let version_str = &bytes[FieldRange::Version];
        if version_str != b" \0" && version_str != b"00" {
            return Err(ParseError::MissingField(String::from(
                "ustar version missing",
            )));
        }
        let version = 0;

        // OWNER
        let owner: Option<String> = utils::get_string(&bytes[FieldRange::Owner]);

        // GROUP
        let group: Option<String> = utils::get_string(&bytes[FieldRange::Group]);

        // MAJOR
        let device_major: Option<u32> = utils::get_number(&bytes[FieldRange::Major]);

        // MINOR
        let device_minor: Option<u32> = utils::get_number(&bytes[FieldRange::Minor]);

        // FILENAME PREFIX
        let filename_prefix: Option<String> = utils::get_string(&bytes[FieldRange::FilenamePrefix]);

        Ok(Some(PosixHeader {
            version,
            owner,
            group,
            device_major,
            device_minor,
            filename_prefix,
        }))
    }
}

/// Its also compatible with the old pre-posix file flag
#[derive(Debug)]
pub enum TypeFlag {
    NormalFile,
    HardLink,
    SymLink,
    CharDev,
    BlockDev,
    Directory,
    Fifo,
    GlobalExtHeader,
    ExtHeader,
}

#[derive(Error, Debug)]
pub enum TypeFlagError {
    #[error("Unknown type flag. This could mean an error or a perhaps a newly standarized flag")]
    UnrecognizedType,

    #[error("Unimplemented vendor extension found")]
    VendorExtension,
}

impl TypeFlag {
    /// Parses a TypeFlag from a byte.
    /// Returns an error for unhandled flags.
    pub fn try_new(byte: u8) -> Result<Self, TypeFlagError> {
        match byte {
            // treat contiguous file as regurar file
            b'\0' | b'0' | b'7' => Ok(TypeFlag::NormalFile),
            b'1' => Ok(TypeFlag::HardLink),
            b'2' => Ok(TypeFlag::SymLink),
            b'3' => Ok(TypeFlag::CharDev),
            b'4' => Ok(TypeFlag::BlockDev),
            b'5' => Ok(TypeFlag::Directory),
            b'6' => Ok(TypeFlag::Fifo),
            b'g' => Ok(TypeFlag::GlobalExtHeader),
            b'x' => Ok(TypeFlag::ExtHeader),
            b'A'..=b'Z' => Err(TypeFlagError::VendorExtension),
            _ => Err(TypeFlagError::UnrecognizedType),
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[test]
    fn parse_header() {
        let header_bytes: [u8; RECORD_SIZE] = [
            0x64, 0x65, 0x76, 0x6c, 0x6f, 0x67, 0x2e, 0x74, 0x78, 0x74, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x30, 0x30, 0x30, 0x30, 0x36, 0x34, 0x34, 0x00, 0x30, 0x30, 0x30, 0x31,
            0x37, 0x35, 0x30, 0x00, 0x30, 0x30, 0x30, 0x31, 0x37, 0x35, 0x30, 0x00, 0x30, 0x30,
            0x30, 0x30, 0x30, 0x30, 0x30, 0x31, 0x36, 0x33, 0x36, 0x00, 0x31, 0x35, 0x31, 0x36,
            0x32, 0x37, 0x37, 0x32, 0x36, 0x35, 0x35, 0x00, 0x30, 0x31, 0x31, 0x35, 0x35, 0x35,
            0x00, 0x20, 0x30, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x75, 0x73, 0x74, 0x61, 0x72, 0x20, 0x20, 0x00, 0x64,
            0x69, 0x72, 0x65, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x64, 0x69, 0x72, 0x65, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ];

        // Contains info for:
        // -rw-r--r-- dire/dire       926 2026-03-31 18:26 devlog.txt
        let header = Header::parse(&header_bytes).unwrap();
        assert_eq!("devlog.txt", header.path.to_string_lossy());
        assert_eq!(926, header.file_size);
        assert_eq!(Some("dire"), header.owner());
        assert_eq!(Some("dire"), header.group());
        assert_eq!("644", format!("{:o}", header.mode.mode()));
    }
}
