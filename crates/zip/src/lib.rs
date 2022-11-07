// https://en.wikipedia.org/wiki/ZIP_(file_format)#Design
// https://pkware.cachefly.net/webdocs/APPNOTE/APPNOTE-6.3.9.TXT

#![allow(dead_code)]

use std::io::SeekFrom;

use tokio::{fs::{self, File}, io::{AsyncSeekExt, AsyncReadExt}};

mod header;

pub(crate) use header::*;

/// Buffer Read Size
const BUFFER_SIZE: usize = 1000;

/// Signature takes up 4 bytes.
const SIGNATURE_SIZE: usize = 4;


pub struct Archive {
    file: File,

    index: usize,
    last_read_amount: usize,

    file_cache: FileReaderCache,

    end_header: Option<EndCentralDirHeader>,
}

impl Archive {
    pub async fn open() -> Self {
        let mut this = Self {
            file: fs::OpenOptions::new().read(true).open("./resources/Zip Test 7-Zip.zip").await.unwrap(),

            index: 0,
            last_read_amount: 0,

            file_cache: FileReaderCache::default(),
            end_header: None,
        };

        {
            let old_pos = this.file.stream_position().await.unwrap();
            let len = this.file.seek(SeekFrom::End(0)).await.unwrap();

            // Avoid seeking a third time when we were already at the end of the
            // stream. The branch is usually way cheaper than a seek operation.
            if old_pos != len {
                this.file.seek(SeekFrom::Start(old_pos)).await.unwrap();
            }
        }

        this.parse().await;

        this
    }

    pub fn info(&self) -> ArchiveInfo {
        self.end_header.as_ref().unwrap().into()
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

    //


    async fn parse(&mut self) {
        self.end_header = Some(EndCentralDirHeader::find(self).await);

        // A directory is placed at the end of a ZIP file. This identifies what files are in the ZIP and identifies where in the ZIP that file is located.
        // A ZIP file is correctly identified by the presence of an end of central directory record which is located at the end of the archive structure in order to allow the easy appending of new files.
        // The order of the file entries in the central directory need not coincide with the order of file entries in the archive.

        // Seek back to start.
        self.file.seek(SeekFrom::Start(0)).await.unwrap();
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

    fn skip<const COUNT: usize>(&mut self) {
        self.index += COUNT;
    }

    async fn get_next_chunk<'a, const COUNT: usize>(&mut self, buffer: &'a mut [u8; BUFFER_SIZE]) -> &'a [u8] {
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