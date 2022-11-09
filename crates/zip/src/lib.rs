// https://en.wikipedia.org/wiki/ZIP_(file_format)#Design
// https://pkware.cachefly.net/webdocs/APPNOTE/APPNOTE-6.3.9.TXT
// https://users.cs.jmu.edu/buchhofp/forensics/formats/pkzip.html

#![feature(iter_array_chunks)]

#![allow(dead_code)]

#![deny(
    clippy::unwrap_used,
    clippy::expect_used
)]


use std::{io::SeekFrom, path::Path};

use tokio::{fs::{self, File}, io::{AsyncSeekExt, AsyncReadExt}};

mod compression;
mod error;
mod header;

pub(crate) use header::*;
pub use error::*;
pub use compression::CompressionType;

/// Buffer Read Size
const BUFFER_SIZE: usize = 1000;

/// Signature takes up 4 bytes.
const SIGNATURE_SIZE: usize = 4;


pub struct Archive {
    file: File,

    file_cache: FileReaderCache,

    end_header: EndCentralDirHeader,
}

impl Archive {
    pub async fn open(path: impl AsRef<Path>) -> Result<Self> {
        let mut this = Self {
            file: fs::OpenOptions::new().read(true).open(path).await?,

            file_cache: FileReaderCache::default(),
            end_header: EndCentralDirHeader::default(),
        };

        this.parse().await?;

        // TODO: Move out. Capacity reserve is used to tell us how many files we have for when we iterate through.
        this.file_cache.files.reserve(this.end_header.total_record_count as usize);

        Ok(this)
    }

    pub fn info(&self) -> ArchiveInfo {
        (&self.end_header).into()
    }

    pub async fn read_file(&mut self) {
        // let file = &self.files[4];

        // This is the offset from the start of the first disk on
        // which this file appears, to where the local header SHOULD
        // be found.  If an archive is in ZIP64 format and the value
        // in this field is 0xFFFFFFFF, the size will be in the
        // corresponding 8 byte zip64 extended information extra field.

        // let mut reader = ArchiveReader::init(&mut self.file).await?;
        // LocalFileHeader::parse(self, self.files[2].relative_offset as u64).await;
    }

    pub async fn iter_files(&mut self) {
        //
    }

    pub async fn list_files(&mut self) -> Result<Vec<CentralDirHeader>> {
        let mut reader = ArchiveReader::init(&mut self.file).await?;

        self.file_cache.list_files(&mut reader).await
    }


    async fn parse(&mut self) -> Result<()> {
        let mut reader = ArchiveReader::init(&mut self.file).await?;

        self.end_header = EndCentralDirHeader::find(&mut reader).await?;

        // A directory is placed at the end of a ZIP file. This identifies what files are in the ZIP and identifies where in the ZIP that file is located.
        // A ZIP file is correctly identified by the presence of an end of central directory record which is located at the end of the archive structure in order to allow the easy appending of new files.
        // The order of the file entries in the central directory need not coincide with the order of file entries in the archive.

        Ok(())
    }
}


pub struct ArchiveReader<'a> {
    // TODO: Utilize BufReader
    file: &'a mut File,

    index: usize,
    last_read_amount: usize,
}

impl<'a> ArchiveReader<'a> {
    pub async fn init(file: &'a mut File) -> Result<ArchiveReader<'a>> {
        // Seek back to start.
        file.seek(SeekFrom::Start(0)).await?;

        Ok(Self {
            file,

            index: 0,
            last_read_amount: 0,
        })
    }

    async fn get_seek_position(&mut self) -> Result<u64> {
        // Get the stream position, add our index (overflow fix), then remove buffer size
        Ok(self.file.stream_position().await? + self.index as u64 - self.last_read_amount as u64)
    }

    async fn seek_to_index(&mut self, buffer: &mut [u8; BUFFER_SIZE]) -> Result<()> {
        // No need if we've already loaded in the end.
        if self.last_read_amount != BUFFER_SIZE {
            return Ok(());
        }

        let seek_amount = self.index as i64 - BUFFER_SIZE as i64;

        self.file.seek(SeekFrom::Current(seek_amount)).await?;
        self.seek_next(buffer).await?;

        Ok(())
    }

    /// Repopulate buffer with next data. read also updates seek position.
    async fn seek_next(&mut self, buffer: &mut [u8; BUFFER_SIZE]) -> Result<()> {
        self.last_read_amount = self.file.read(buffer).await?;
        self.index = 0;

        Ok(())
    }

    /// Repopulate buffer with next data. read also updates seek position.
    async fn seek_to(&mut self, value: u64) -> Result<()> {
        self.file.seek(SeekFrom::Start(value)).await?;
        self.index = 0;

        Ok(())
    }

    fn skip<const COUNT: usize>(&mut self) {
        self.index += COUNT;
    }

    async fn get_next_chunk<'b, const COUNT: usize>(&mut self, buffer: &'b mut [u8; BUFFER_SIZE]) -> Result<&'b [u8]> {
        if self.index + COUNT >= buffer.len() {
            self.seek_to_index(buffer).await?;
        }

        let v = &buffer[self.index..self.index + COUNT];

        self.index += COUNT;

        Ok(v)
    }

    async fn get_chunk_amount(&mut self, buffer: &mut [u8; BUFFER_SIZE], mut size: usize) -> Result<Vec<u8>> {
        let mut filled = Vec::with_capacity(size);

        while size != 0 {
            if self.index + size >= buffer.len() {
                // println!("idx {}, size {} >= buffer {}", self.index, size, buffer.len());
                filled.extend_from_slice(&buffer[self.index..]);

                size -= buffer.len() - self.index;

                self.seek_next(buffer).await?;
            } else {
                filled.extend_from_slice(&buffer[self.index..self.index + size]);
                self.index += size;

                break;
            }
        }

        Ok(filled)
    }

    fn find_next_signature<const SIG_SIZE: usize>(&self, buffer: &[u8], signature: [u8; SIG_SIZE]) -> Option<usize> {
        buffer[self.index..].windows(SIG_SIZE)
            .position(|v| v == signature)
            .map(|offset| self.index + offset)
    }

    async fn next_u16(&mut self, buffer: &mut [u8; BUFFER_SIZE]) -> Result<u16> {
        let buf = self.get_next_chunk::<2>(buffer).await?;

        Ok((buf[1] as u16) << 8 | buf[0] as u16)
    }

    async fn next_u32(&mut self, buffer: &mut [u8; BUFFER_SIZE]) -> Result<u32> {
        let buf = self.get_next_chunk::<4>(buffer).await?;

        Ok((buf[3] as u32) << 24 | (buf[2] as u32) << 16 | (buf[1] as u32) << 8 | buf[0] as u32)
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::expect_used)]

    use tokio::runtime::Runtime;

    use super::*;

    #[test]
    fn zip_7zip_default() -> Result<(), Error> {
        let rt = Runtime::new()?;

        rt.block_on(async {
            let mut archive = Archive::open("./resources/Zip Test 7-Zip.zip").await?;

            //

            Result::<_, Error>::Ok(())
        })?;

        Ok(())
    }

    #[test]
    fn zip_windows_default() -> Result<(), Error> {
        let rt = Runtime::new()?;

        rt.block_on(async {
            let mut archive = Archive::open("./resources/Zip Test Windows.zip").await?;

            //

            Result::<_, Error>::Ok(())
        })?;

        Ok(())
    }
}