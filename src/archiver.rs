use std::{
    fs::File,
    io::{Read, Seek},
    os::unix,
    path::{Path, PathBuf},
};

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
    header::{Header, TypeFlag},
    io,
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

        loop {
            let mut record_buf: [u8; RECORD_SIZE] = [0; RECORD_SIZE];

            file.read_exact(&mut record_buf)
                .expect("Malformed tar archive");

            let Ok(header) = Header::parse(&record_buf) else {
                // TODO: error handling
                break;
            };

            // save old offset
            let offset = file.stream_position().unwrap();

            // skip file content
            file.seek_relative(header.file_size() as i64)
                .expect("seek failed");

            let pos = file.stream_position().unwrap();
            let rem = pos as usize % RECORD_SIZE;

            // only align forward if we are not in a position divisible by
            // RECORD_SIZE
            if rem != 0 {
                let align_forward = RECORD_SIZE - rem;
                file.seek_relative(align_forward as i64)
                    .expect("align forward failed");
            }

            files.push(FileInfo { header, offset });
        }

        Self {
            files,
            archive: PathBuf::from(filename),
        }
    }

    /// Extract the archive contents into the given directory
    pub fn extract_to_dir(&mut self, dest: &Path) -> Result<(), std::io::Error> {
        // ensure output dir exists
        if !dest.exists() {
            std::fs::create_dir_all(dest)?;
        }

        // move to the new output dir
        std::env::set_current_dir(dest)?;

        // NOTE: Here I want to rename the original archive file relative to the
        // new dir. Its a bit ugly tho.
        if dest != "." {
            let mut comps = vec![];
            for c in dest.components() {
                comps.push(String::from(".."));
            }
            comps.push(self.archive.to_string_lossy().to_string());
            self.archive = comps.iter().collect();
        }

        for file in &self.files {
            self.extract_object(file, dest)?;
            eprintln!(
                "Extracted {}/{}",
                dest.display(),
                file.header.path().display()
            );
        }

        Ok(())
    }

    /// Extracts the given objecto to its corresponding file or directory.
    ///
    /// Is responsible for copying the contents from the archive and ensuring that
    /// the created file has the correct metadata.
    fn extract_object(&self, obj: &FileInfo, dest: &Path) -> Result<(), std::io::Error> {
        match obj.header.file_type() {
            TypeFlag::NormalFile => {
                io::copy_file_range(
                    self.archive.as_path(),
                    obj.offset,
                    obj.header.file_size(),
                    obj.header.path().as_path(),
                    0,
                )?;
            }
            TypeFlag::HardLink => {
                let original = obj.header.path();

                if original.exists() {
                    let link: &str = obj
                        .header
                        .linked_file()
                        .expect("must be present for hardlinks");
                    std::fs::hard_link(link, original)?;
                }
            }
            TypeFlag::SymLink => {
                let target: &str = obj
                    .header
                    .linked_file()
                    .expect("must be present for symlinks");
                unix::fs::symlink(target, obj.header.path())?;
            }
            TypeFlag::Directory => {
                std::fs::create_dir_all(obj.header.path())?;
            }
            _ => todo!("extract rest of file types"),
        }

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
                file.file_type().into(),
                format!("{} B", file.file_size()),
                file.permissions(),
                file.modified(),
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
        assert_eq!(tar.files.len(), 20931);
    }
}
