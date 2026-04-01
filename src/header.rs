use std::{
    fs,
    num::ParseIntError,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

use crate::RECORD_SIZE;
use chrono::{DateTime, TimeZone, Utc};
use thiserror::Error;

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

mod util {
    /// Helper to convert bytes into valid str slices.
    ///
    /// Panics.
    ///
    pub fn bytes_to_str(bytes: &[u8]) -> &str {
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
    pub fn rstrip(bytes: &[u8]) -> Option<&[u8]> {
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
}

impl Header {
    pub fn parse(bytes: &[u8; RECORD_SIZE]) -> Result<Self, ParseError> {
        // PATH
        let path_bytes =
            util::rstrip(&bytes[0..100]).ok_or(ParseError::MissingField("path".into()))?;
        let path = PathBuf::from(util::bytes_to_str(path_bytes));

        // MODE
        let mode_bytes =
            util::rstrip(&bytes[100..108]).ok_or(ParseError::MissingField("mode".into()))?;
        let mode_str = util::bytes_to_str(mode_bytes);
        let mode_raw = u32::from_str_radix(mode_str, 8).expect("failed to parse mode");
        let mode = fs::Permissions::from_mode(mode_raw);

        // UID
        let uid_bytes =
            util::rstrip(&bytes[108..116]).ok_or(ParseError::MissingField("uid".into()))?;
        let uid_str = util::bytes_to_str(uid_bytes);
        let uid = u32::from_str_radix(uid_str, 8).expect("failed to parse uid");

        // GID
        let gid_bytes =
            util::rstrip(&bytes[116..124]).ok_or(ParseError::MissingField("gid".into()))?;
        let gid_str = util::bytes_to_str(gid_bytes);
        let gid = u32::from_str_radix(gid_str, 8).expect("failed to parse gid");

        // FILE SIZE
        let file_size_bytes =
            util::rstrip(&bytes[124..136]).ok_or(ParseError::MissingField("file size".into()))?;
        let file_size_str = util::bytes_to_str(file_size_bytes);
        let file_size = u64::from_str_radix(file_size_str, 8)?;

        // MTIME
        let mtime_bytes =
            util::rstrip(&bytes[136..148]).ok_or(ParseError::MissingField("mtime".into()))?;
        let mtime_str = util::bytes_to_str(mtime_bytes);
        let mtime_seconds = i64::from_str_radix(mtime_str, 8)?;
        let mtime = Utc
            .timestamp_opt(mtime_seconds, 0)
            .single()
            .expect("Invalid mtime unix time");

        // CHECKSUM
        let checksum_bytes =
            util::rstrip(&bytes[148..156]).ok_or(ParseError::MissingField("checksum".into()))?;
        let checksum_str = util::bytes_to_str(checksum_bytes);
        let checksum = u64::from_str_radix(checksum_str, 8)?;

        // TYPE FLAG
        let flag_byte = &bytes[156..157];
        let type_flag = TypeFlag::try_new(flag_byte[0])?;

        // LINKED FILE (optional)
        let linked_file = util::rstrip(&bytes[157..257]).and_then(|linked| {
            let link_str = util::bytes_to_str(linked);
            Some(String::from(link_str))
        });

        // POSIX HEADER (optional, common case)
        let ustar_magic = &bytes[257..263];
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

        if header.verify_checksum() {
            Ok(header)
        } else {
            Err(ParseError::ChecksumFailed)
        }
    }

    pub fn filename(&self) -> &Path {
        self.path.as_path()
    }

    /// The checksum is calculated by taking the sum of the {un}signed byte
    /// values of the header record with the eight checksum bytes taken to
    /// be ASCII spaces (decimal value 32). It is stored as a six digit octal
    /// number with leading zeroes followed by a NUL and then a space.
    fn verify_checksum(&self) -> bool {
        // TODO: verify checksum
        true
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
        let ustar_magic = &bytes[257..263];

        if ustar_magic != b"ustar " && ustar_magic != b"ustar\0" {
            return Ok(None);
        }

        // FIXME: is this a number or string???
        // let version = &bytes[263..265];
        let version: u16 = 0;

        // OWNER (optional)
        let owner = util::rstrip(&bytes[265..297]).and_then(|user_bytes| {
            let user_str = util::bytes_to_str(user_bytes);
            Some(String::from(user_str))
        });

        // GROUP (optional)
        let group = util::rstrip(&bytes[297..329]).and_then(|user_bytes| {
            let user_str = util::bytes_to_str(user_bytes);
            Some(String::from(user_str))
        });

        // MAJOR (optional)
        let device_major = util::rstrip(&bytes[329..337]).and_then(|v| {
            let v = util::bytes_to_str(v);
            let version = u32::from_str_radix(v, 8).ok()?;
            Some(version)
        });

        // MINOR (optional)
        let device_minor = util::rstrip(&bytes[337..345]).and_then(|v| {
            let v = util::bytes_to_str(v);
            let version = u32::from_str_radix(v, 8).ok()?;
            Some(version)
        });

        // FILENAME PREFIX (optional)
        let filename_prefix = util::rstrip(&bytes[345..500]).and_then(|p| {
            let prefix = util::bytes_to_str(p);
            Some(String::from(prefix))
        });

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
    ContiguousFile, // Same as normal file
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
            b'\0' | b'0' => Ok(TypeFlag::NormalFile),
            b'1' => Ok(TypeFlag::HardLink),
            b'2' => Ok(TypeFlag::SymLink),
            b'3' => Ok(TypeFlag::CharDev),
            b'4' => Ok(TypeFlag::BlockDev),
            b'5' => Ok(TypeFlag::Directory),
            b'6' => Ok(TypeFlag::Fifo),
            b'7' => Ok(TypeFlag::ContiguousFile),
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
            0x00, 0x00, 0x30, 0x31, 0x30, 0x30, 0x36, 0x34, 0x34, 0x00, 0x30, 0x30, 0x30, 0x31,
            0x37, 0x35, 0x30, 0x00, 0x30, 0x30, 0x30, 0x31, 0x37, 0x35, 0x30, 0x00, 0x30, 0x30,
            0x30, 0x30, 0x30, 0x30, 0x30, 0x31, 0x32, 0x36, 0x37, 0x00, 0x31, 0x35, 0x31, 0x36,
            0x32, 0x36, 0x31, 0x30, 0x36, 0x30, 0x34, 0x00, 0x30, 0x30, 0x31, 0x31, 0x32, 0x36,
            0x37, 0x00, 0x30, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x75, 0x73, 0x74, 0x61, 0x72, 0x20, 0x20, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30,
            0x00, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x30, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
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
        // -rw-r--r-- 1000/1000       695 2026-03-31 02:12 devlog.txt

        let header = Header::parse(&header_bytes).unwrap();
        // TODO: keep testing and asserting
        assert_eq!("devlog.txt", header.filename());

        println!("{:?}", header);
    }
}
