// https://en.wikipedia.org/wiki/ZIP_(file_format)#Design
// https://pkware.cachefly.net/webdocs/APPNOTE/APPNOTE-6.3.9.TXT

#![allow(dead_code)]

use std::{io::SeekFrom, path::Path};

use tokio::{fs::{self, File}, io::{AsyncSeekExt, AsyncReadExt}};

mod header;

pub(crate) use header::*;

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
    pub async fn open(path: impl AsRef<Path>) -> Self {
        let mut this = Self {
            file: fs::OpenOptions::new().read(true).open(path).await.unwrap(),

            file_cache: FileReaderCache::default(),
            end_header: EndCentralDirHeader::default(),
        };

        this.parse().await;

        // TODO: Move out. Capacity reserve is used to tell us how many files we have for when we iterate through.
        this.file_cache.files.reserve(this.end_header.total_record_count as usize);

        this
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

        // LocalFileHeader::parse(self, self.files[2].relative_offset as u64).await;
    }

    pub async fn iter_files(&mut self) {
        //
    }

    pub async fn list_files(&mut self) -> Vec<CentralDirHeader> {
        let mut reader = ArchiveReader::init(&mut self.file).await;

        self.file_cache.list_files(&mut reader).await
    }


    async fn parse(&mut self) {
        let mut reader = ArchiveReader::init(&mut self.file).await;

        self.end_header = EndCentralDirHeader::find(&mut reader).await;

        // A directory is placed at the end of a ZIP file. This identifies what files are in the ZIP and identifies where in the ZIP that file is located.
        // A ZIP file is correctly identified by the presence of an end of central directory record which is located at the end of the archive structure in order to allow the easy appending of new files.
        // The order of the file entries in the central directory need not coincide with the order of file entries in the archive.
    }
}


pub struct ArchiveReader<'a> {
    file: &'a mut File,

    index: usize,
    last_read_amount: usize,
}

impl<'a> ArchiveReader<'a> {
    pub async fn init(file: &'a mut File) -> ArchiveReader<'a> {
        // Seek back to start.
        file.seek(SeekFrom::Start(0)).await.unwrap();

        Self {
            file,

            index: 0,
            last_read_amount: 0,
        }
    }

    async fn get_seek_position(&mut self) -> u64 {
        // Get the stream position, add our index (overflow fix), then remove buffer size
        self.file.stream_position().await.unwrap() + self.index as u64 - self.last_read_amount as u64
    }

    async fn seek_to_index(&mut self, buffer: &mut [u8; BUFFER_SIZE]) {
        // No need if we've already loaded in the end.
        if self.last_read_amount != BUFFER_SIZE {
            return;
        }

        let seek_amount = self.index as i64 - BUFFER_SIZE as i64;

        self.file.seek(SeekFrom::Current(seek_amount)).await.unwrap();
        self.seek_next(buffer).await;
    }

    /// Repopulate buffer with next data. read also updates seek position.
    async fn seek_next(&mut self, buffer: &mut [u8; BUFFER_SIZE]) {
        self.last_read_amount = self.file.read(buffer).await.unwrap();
        self.index = 0;
    }

    /// Repopulate buffer with next data. read also updates seek position.
    async fn seek_to(&mut self, value: u64) {
        self.file.seek(SeekFrom::Start(value)).await.unwrap();
        self.index = 0;
    }

    fn skip<const COUNT: usize>(&mut self) {
        self.index += COUNT;
    }

    async fn get_next_chunk<'b, const COUNT: usize>(&mut self, buffer: &'b mut [u8; BUFFER_SIZE]) -> &'b [u8] {
        if self.index + COUNT >= buffer.len() {
            self.seek_to_index(buffer).await;
        }

        let v = &buffer[self.index..self.index + COUNT];

        self.index += COUNT;

        v
    }

    async fn get_chunk_amount(&mut self, buffer: &mut [u8; BUFFER_SIZE], mut size: usize) -> Vec<u8> {
        let mut filled = Vec::with_capacity(size);

        while size != 0 {
            if self.index + size >= buffer.len() {
                // println!("idx {}, size {} >= buffer {}", self.index, size, buffer.len());
                filled.extend_from_slice(&buffer[self.index..]);

                size -= filled.len();

                self.seek_next(buffer).await;
            } else {
                filled.extend_from_slice(&buffer[self.index..self.index + size]);
                self.index += size;

                break;
            }
        }

        filled
    }

    fn find_next_signature<const SIG_SIZE: usize>(&self, buffer: &[u8], signature: [u8; SIG_SIZE]) -> Option<usize> {
        buffer[self.index..].windows(SIG_SIZE)
            .position(|v| v == signature)
            .map(|offset| self.index + offset)
    }

    async fn next_u16(&mut self, buffer: &mut [u8; BUFFER_SIZE]) -> u16 {
        let buf = self.get_next_chunk::<2>(buffer).await;

        (buf[1] as u16) << 8 | buf[0] as u16
    }

    async fn next_u32(&mut self, buffer: &mut [u8; BUFFER_SIZE]) -> u32 {
        let buf = self.get_next_chunk::<4>(buffer).await;

        (buf[3] as u32) << 24 | (buf[2] as u32) << 16 | (buf[1] as u32) << 8 | buf[0] as u32
    }
}