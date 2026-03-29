use std::{error::Error, io::BufRead};

pub fn list_archive(mut file: impl BufRead) -> Result<(), Box<dyn Error>> {
    let mut v: Vec<u8> = Vec::with_capacity(1000);
    let r = file.read(&mut v)?;
    println!("read {r} bytes");
    Ok(())
}

#[repr(u8)]
enum TypeFlag {
    ///
    NormalFile(u8) = '0' as u8,
    ///
    HardLink(u8) = '1' as u8,
    ///
    SymLink(u8) = '2' as u8,
    ///
    CharDevice(u8) = '3' as u8,
    ///
    BlockDevice(u8) = '4' as u8,
    ///
    Directory(u8) = '5' as u8,
    ///
    Fifo(u8) = '6' as u8,
    /// Same as normal file
    ContiguousFile(u8) = '7' as u8,
    ///
    GlobalExtHeader(u8) = 'g' as u8,
    ///
    ExtHeader(u8) = 'x' as u8,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn foo() {}
}
