use std::{
    fs::{self, File},
    io::{Read, Seek},
    os::unix::{self},
    path::{Path, PathBuf},
    time::SystemTime,
};

use log::{info, warn};
use tabled::{
    Table,
    builder::Builder,
    settings::{
        Alignment, Color, Style,
        object::{Columns, Rows},
    },
};

use crate::{
    RECORD_SIZE,
    header::{Header, ParseError, TypeFlag},
    utils::{self, io},
};

struct FileInfo {
    /// Contains the parsed metadata of the file
    header: Header,
    /// Byte position into the archive where the file data starts
    offset: u64,
}

pub struct Archiver {
    /// All parsed information of the fiele objects
    files: Vec<FileInfo>,
    /// Name of the tar file
    archive: PathBuf,
}

impl Archiver {
    /// Opens the Tar archive file and parses all file objects.
    pub fn parse(filename: &Path) -> Self {
        let mut file = File::open(filename).expect("cannot open archive file");
        let mut files: Vec<FileInfo> = Vec::new();
        let mut long_path: Option<String> = None; // will handle 'L' extension

        loop {
            let mut record_buf: [u8; RECORD_SIZE] = [0; RECORD_SIZE];
            file.read_exact(&mut record_buf)
                .expect("Malformed tar archive");

            let header = match Header::parse(&record_buf, long_path) {
                Ok(header) => header,
                Err(ParseError::EmptyHeader) => break,
                Err(e) => panic!("{}", e),
            };

            if header.type_flag == TypeFlag::LongPathName {
                let long = extract_long_path(&header, &mut file).unwrap_or_default();
                long_path = Some(long);
                continue;
            } else {
                // Unmark it so it doesnt
                long_path = None;
            }

            // save old offset
            let data_start = file.stream_position().unwrap();
            assert!(data_start.is_multiple_of(RECORD_SIZE as u64));

            // skip file content
            file.seek_relative(header.file_size as i64)
                .expect("seek failed");

            // necessary as file contents might end at any point
            io::align_forward(&mut file);

            files.push(FileInfo {
                header,
                offset: data_start,
            });
        }

        Self {
            files,
            archive: PathBuf::from(filename),
        }
    }

    /// Extract the archive contents into the given directory
    pub fn extract_to_dir(&mut self, dest: &Path, verbose: bool) -> Result<(), std::io::Error> {
        // ensure output dir exists
        if !dest.exists() {
            std::fs::create_dir_all(dest)?;
            if verbose {
                info!("Created output directory '{}'", dest.display());
            }
        }

        // move to the new output dir
        std::env::set_current_dir(dest)?;

        // NOTE: Here I want to rename the original archive file relative to the
        // new dir. Its a bit ugly tho.
        if dest != "." {
            let mut comps = vec![];
            for _ in dest.components() {
                comps.push(String::from(".."));
            }
            comps.push(self.archive.to_string_lossy().to_string());
            self.archive = comps.iter().collect();
        }

        for file in &self.files {
            self.extract_object(file)?;
            if verbose {
                info!("Extracted '{}'", file.header.path().display());
            }
        }

        Ok(())
    }

    /// Extracts the given object to its corresponding file or directory.
    ///
    /// The destination dir is relative to cwd,
    /// for further control use ```extract_to_dir``` function.
    ///
    /// Is responsible for copying the contents from the archive and ensuring that
    /// the created file has the correct metadata.
    fn extract_object(&self, obj: &FileInfo) -> Result<(), std::io::Error> {
        match obj.header.type_flag {
            TypeFlag::NormalFile => {
                let src = self.archive.as_path();
                let dest = obj.header.path();
                io::copy_file_range(src, obj.offset, obj.header.file_size, dest.as_path(), 0)?;
            }
            TypeFlag::HardLink => {
                let link = obj.header.path();
                if link.exists() {
                    let original: &str = obj
                        .header
                        .linked_file
                        .as_deref()
                        .expect("must be present for hardlinks");
                    std::fs::hard_link(original, link)?;
                }
            }
            TypeFlag::SymLink => {
                let link = obj.header.path();
                let original: &str = obj
                    .header
                    .linked_file
                    .as_deref()
                    .expect("must be present for symlinks");
                unix::fs::symlink(original, link)?;
            }
            TypeFlag::Directory => {
                let dir = obj.header.path();
                std::fs::create_dir_all(dir)?;
            }
            _ => todo!("extract rest of file types"),
        }

        // Update new file/dir/symlink metadata
        let dest = obj.header.path();

        update_metadata(
            dest.as_path(),
            obj.header.mtime.into(),
            obj.header.mode.clone(), // cheap
            obj.header.uid,
            obj.header.gid,
        )?;

        Ok(())
    }

    /// Pretty print all files contained in the archive in form of
    /// table.
    pub fn print_files(&self, verbose: bool) {
        if !verbose {
            let mut data = vec![vec![String::from("#"), String::from("name")]];
            for (i, f) in self.files.iter().map(|fi| &fi.header).enumerate() {
                data.push(vec![format!("{i}"), f.display_name()]);
            }
            let mut table = Table::from_iter(data);
            table.with(Style::rounded());
            table.with(Color::FG_GREEN);
            println!("{table}");
            return;
        }

        let mut build = Builder::default();
        build.push_record(["#", "name", "type", "size", "mode", "modified"]);

        for (index, file) in self.files.iter().map(|fi| &fi.header).enumerate() {
            build.push_record([
                format!("{index}"),
                file.display_name(),
                file.type_flag.into(),
                file.display_size(),
                file.display_permissions(),
                file.display_modified(),
            ]);
        }

        let mut table = build.build();
        // little hack to make it look like nushell's one <3
        table
            .with(Style::rounded())
            .modify(Columns::first(), Color::FG_GREEN | Color::BOLD)
            .modify(Columns::one(1), Color::FG_GREEN)
            .modify(Columns::one(3), Color::FG_CYAN)
            .modify(Columns::one(4), Color::FG_MAGENTA)
            .modify(Columns::one(3), Alignment::right())
            .modify(Columns::one(4), Alignment::right())
            .modify(Rows::first(), Color::FG_GREEN | Color::BOLD)
            .modify(Rows::first(), Alignment::center());

        println!("{table}");
    }
}

/// Updates the metadata for the given file
fn update_metadata(
    path: &Path,
    modified: SystemTime,
    mode: fs::Permissions,
    uid: u32,
    gid: u32,
) -> Result<(), std::io::Error> {
    let f = File::open(path)?;

    if path.is_symlink() {
        _ = unix::fs::lchown(path, Some(uid), Some(gid))
            .map_err(|err| warn!("Failed to update uid/gid: {}", err));
    } else {
        _ = unix::fs::chown(path, Some(uid), Some(gid))
            .map_err(|err| warn!("Failed to update uid/gid: {}", err));
    }

    f.set_permissions(mode)?;
    f.set_modified(modified)?;
    Ok(())
}

/// Takes care of parsing the long path name described in
/// the 'L' extension of the file flag
///
/// more info: `https://stackoverflow.com/questions/2078778/what-exactly-is-the-gnu-tar-longlink-trick`
fn extract_long_path(header: &Header, file: &mut File) -> Result<String, std::io::Error> {
    let (long_link, long_path_len) = (&header.path, header.file_size);

    // GNU tar convention is to mark the name with this
    assert!(long_link == "././@LongLink");

    // read the extended file name
    let mut extended_path: Vec<u8> = vec![0; long_path_len as usize];
    file.read_exact(&mut extended_path)?;

    // align to block boundary and parse the next header
    io::align_forward(file);

    // mark the path so it can be used for the next file object
    utils::get_string(&extended_path).ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "Long file name is not a valid string",
        )
    })
}

#[cfg(test)]
mod tests {
    use crate::archiver::Archiver;
    use std::path::Path;

    #[test]
    fn printing_archive() {
        let tar = Archiver::parse(Path::new(
            "/home/dire/Documents/Coding/github/tar/test-data/zig_0_16_0.tar",
        ));

        // TODO: pass this test
        assert_eq!(tar.files.len(), 20932);
    }
}
