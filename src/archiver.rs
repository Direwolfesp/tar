use std::{
    fs::File,
    io::{Read, Seek},
    path::Path,
};

use tabled::{
    builder::Builder,
    settings::{
        Alignment, Color, Style,
        object::{Columns, Rows},
    },
};

use crate::{RECORD_SIZE, header::Header};

struct FileInfo {
    header: Header,
    offset: u64,
}

pub struct Archiver {
    files: Vec<FileInfo>,
    source: File,
}

impl Archiver {
    /// Opens the Tar archive file and parses all file objects.
    pub fn parse(filename: &Path) -> Self {
        let mut source = File::open(filename).expect("cannot open archive file");
        let mut files: Vec<FileInfo> = Vec::new();

        loop {
            let mut record_buf: [u8; RECORD_SIZE] = [0; RECORD_SIZE];

            source
                .read_exact(&mut record_buf)
                .expect("Malformed tar archive");

            let Ok(header) = Header::parse(&record_buf) else {
                // TODO: error handling
                break;
            };

            // save old offset
            let offset = source.stream_position().unwrap();

            // skip file content
            source
                .seek_relative(header.file_size() as i64)
                .expect("seek failed");

            let pos = source.stream_position().unwrap();
            let rem = pos as usize % RECORD_SIZE;

            // only align forward if we are not in a position divisible by
            // RECORD_SIZE
            if rem != 0 {
                let align_forward = RECORD_SIZE - rem;
                source
                    .seek_relative(align_forward as i64)
                    .expect("align forward failed");
            }

            files.push(FileInfo { header, offset });
        }

        Self { files, source }
    }

    /// Pretty print all files contained in the archive in the form of
    /// table.
    pub fn print_files(&self) {
        let mut build = Builder::default();
        build.push_record(["#", "name", "type", "size", "mode", "modified"]);

        for (index, file) in self.files.iter().map(|fi| &fi.header).enumerate() {
            build.push_record([
                format!("{index}"),
                file.file_name().display().to_string(),
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
    use crate::archiver;
    use std::path::Path;

    #[test]
    fn printing_archive() {
        let tar = archiver::Archiver::parse(Path::new(
            "/home/dire/Documents/Coding/github/tar/other.tar",
        ));

        tar.print_files();
    }
}
