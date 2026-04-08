use std::{fmt::Debug, fs, num::ParseIntError, os::unix::fs::PermissionsExt, path::PathBuf};

use crate::{RECORD_SIZE, utils};

use chrono::{DateTime, Local, TimeZone, Utc};
use thiserror::Error;

/// Generic tar header structure that tries to comply with both legacy and modern
/// POSIX format record.
#[derive(Debug)]
pub struct Header {
    /// File path and name
    pub path: PathBuf,

    /// File mode (octal)
    pub mode: fs::Permissions,

    /// Owner's numeric user ID (octal)
    pub uid: u32,

    /// Group's numeric user ID (octal)
    pub gid: u32,

    /// File size in bytes (octal)
    pub file_size: u64,

    /// Last modification time in numeric Unix time format (octal)
    pub mtime: DateTime<Utc>,

    /// Checksum for header record
    checksum: u64,

    /// File type
    pub type_flag: TypeFlag,

    /// Name of linked file.
    ///
    /// If several files with the same name appear in a tar archive, only the first one is
    /// archived as a normal file; the rest are archived as hard links, with the
    /// "name of linked file" field set to the first one's name. On extraction,
    /// such hard links should be recreated in the file system.
    pub linked_file: Option<String>,

    /// Only present for post 1988 POSIX IEEE standard archives. Which is almost the case
    /// nowadays. Must check for the presence of "ustar\0" at offset 257
    posix_header: Option<PosixHeader>,
}

#[derive(Debug, Error)]
pub enum ParseError {
    #[error("Failed to parse octal ascii number: {0}")]
    InvalidNumber(#[from] ParseIntError),

    #[error("File object contains and invalid type flag: {0}")]
    BadFileTypeFlag(#[from] TypeFlagError),

    #[error("Missing primary field: {0}")]
    MissingField(String),

    #[error("Failed to verify header, is the file corrupted?")]
    ChecksumFailed,

    #[error("Header is empty, usually means EOF")]
    EmptyHeader,
}

impl Header {
    pub fn parse(bytes: &[u8; RECORD_SIZE], long_path: Option<String>) -> Result<Self, ParseError> {
        if bytes.iter().all(|&b| b == 0) {
            return Err(ParseError::EmptyHeader);
        }

        // PATH
        let path_string = utils::get_string(&bytes[FieldRange::Path])
            .ok_or(ParseError::MissingField("path".into()))?;
        let mut path = PathBuf::from(path_string);

        // If long path is present (L extension), we must
        // use this new long path instead of the old one, as
        // it will be truncated to the first 100 characters
        if let Some(long_path) = long_path {
            path.clear();
            path.push(long_path.trim_ascii());
        }

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
            None
        }
    }

    /// get file owner group name
    pub fn group(&self) -> Option<&str> {
        if let Some(posix) = &self.posix_header {
            posix.group.as_deref()
        } else {
            None
        }
    }

    /// If present, this must be prepended before the regular path name
    fn prefix(&self) -> Option<&str> {
        if let Some(posix) = &self.posix_header {
            posix.filename_prefix.as_deref()
        } else {
            None
        }
    }

    /// Constructs the path name of the file, taking into account
    /// for extra prefixex that might be present if the filename is too large
    pub fn path(&self) -> PathBuf {
        let mut path = PathBuf::new();

        if let Some(prefix) = self.prefix() {
            path.push(prefix);
        }

        path.push(&self.path);

        path
    }

    /// Formats file size in the appropiate filesize unit: B, KB, MB, GB
    pub fn display_size(&self) -> String {
        let size = self.file_size;
        if size >= 1_000_000_000 {
            format!("{:.1} GB", size as f64 / 1_000_000_000 as f64)
        } else if size >= 1_000_000 {
            format!("{:.1} MB", size as f64 / 1_000_000 as f64)
        } else if size >= 1_000 {
            format!("{:.1} KB", size as f64 / 1000 as f64)
        } else {
            format!("{} B", size)
        }
    }

    /// Get file path formated as string
    pub fn display_name(&self) -> String {
        let path = self.path().as_path().display().to_string();
        match self.type_flag {
            TypeFlag::SymLink => {
                let target = self
                    .linked_file
                    .as_ref()
                    .expect("must be present for symlinks");
                format!("{} -> {}", path, target)
            }
            _ => path,
        }
    }

    /// Formats the file mtime with the local timezone
    pub fn display_modified(&self) -> String {
        let local_time: DateTime<Local> = self.mtime.into();
        local_time.format("%d/%m/%Y %H:%M").to_string()
    }

    /// Formats the file permission bits with the classical
    /// rwx representation
    pub fn display_permissions(&self) -> String {
        let mut mode_fmt = String::with_capacity(9);
        let mode = self.mode.mode();

        for i in (0..=8).rev() {
            let bit = 1 << i;
            if mode & bit != 0 {
                mode_fmt.push(match i % 3 {
                    2 => 'r',
                    1 => 'w',
                    0 => 'x',
                    _ => unreachable!(),
                });
            } else {
                mode_fmt.push('-');
            }
        }

        mode_fmt
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

        checksum as i64 == sum_signed || checksum == sum_unsigned
    }
}

#[allow(dead_code)]
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

/// Its also compatible with the old pre-posix file flag
#[derive(Debug, PartialEq, Copy, Clone)]
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
    LongPathName,
}

impl From<TypeFlag> for String {
    fn from(val: TypeFlag) -> Self {
        match val {
            TypeFlag::HardLink | TypeFlag::NormalFile => "file".into(),
            TypeFlag::SymLink => "symlink".into(),
            TypeFlag::CharDev => "char device".into(),
            TypeFlag::BlockDev => "block device".into(),
            TypeFlag::Directory => "dir".into(),
            TypeFlag::Fifo => "fifo".into(),
            TypeFlag::LongPathName => "???".into(),
            TypeFlag::GlobalExtHeader => unimplemented!(),
            TypeFlag::ExtHeader => unimplemented!(),
        }
    }
}

#[derive(Error, Debug)]
pub enum TypeFlagError {
    #[error("Unknown type flag '{0}'. Could mean an error or a perhaps a newly standarized flag")]
    UnrecognizedType(u8),

    #[error("Unimplemented vendor extension '{0}'")]
    VendorExtension(u8),
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
            b'L' => Ok(TypeFlag::LongPathName),
            b'A'..=b'Z' => Err(TypeFlagError::VendorExtension(byte)),
            _ => Err(TypeFlagError::UnrecognizedType(byte)),
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use chrono::Datelike;

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
        let header = Header::parse(&header_bytes, None).unwrap();
        assert_eq!("devlog.txt", header.path.to_string_lossy());
        assert_eq!(926, header.file_size);
        assert_eq!(Some("dire"), header.owner());
        assert_eq!(Some("dire"), header.group());
        assert_eq!("644", format!("{:o}", header.mode.mode()));
        assert_eq!(2026, header.mtime.year());
        assert_eq!(3, header.mtime.month());
        assert_eq!(31, header.mtime.day());
    }
}
