// https://www.rarlab.com/technote.htm
// https://forensicswiki.xyz/wiki/index.php?title=RAR
// http://acritum.com/winrar/rar-format

#![feature(iter_array_chunks)]

#![allow(dead_code)]

#![deny(
    clippy::unwrap_used,
    clippy::expect_used
)]


use std::{io::SeekFrom, path::Path};

use tokio::{fs::{self, File}, io::{AsyncSeekExt, AsyncReadExt}};
use tracing::debug;

mod error;
mod header;
mod header_4;

pub(crate) use header::*;
pub (crate) use header_4::*;
pub use error::*;


/// Buffer Read Size
const BUFFER_SIZE: usize = 1000;

pub(crate) const SIGNATURE_SIZE: usize = 7;

pub enum Archive {
    Five {
        file: File,

        main_archive: MainArchiveHeader,
        // TODO: Remove. Only store if file contains less than X files. We'll store file name, size, header position instead.
        files: Vec<FileArchiveHeader>,
        end_of_archive: EndOfArchiveHeader,
    },

    Four {
        file: File,
    }
}

impl Archive {
    pub async fn open(path: impl AsRef<Path>) -> Result<Self> {
        let file = fs::OpenOptions::new().read(true).open(path).await?;

        Self::parse(file).await
    }

    // pub fn info(&self) -> ArchiveInfo {
    //     (&self.end_header).into()
    // }

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

    // pub async fn list_files(&mut self) -> Result<Vec<CentralDirHeader>> {
    //     let mut reader = ArchiveReader::init(&mut self.file).await?;

    //     self.files.list_files(&mut reader).await
    // }


    async fn parse(mut file: File) -> Result<Self> {
        let mut reader = ArchiveReader::init(&mut file, false).await?;

        let mut buffer = [0u8; BUFFER_SIZE];

        let mut main_archive = None;
        let mut files = Vec::new();
        let mut end_of_archive = None;

        loop {
            // Read updates seek position
            reader.last_read_amount = reader.file.read(&mut buffer).await?;
            reader.index = 0;

            if let Some(at_index) = reader.find_signature_pos(&buffer) {
                // TODO: Handle Self-extracting module before signature.

                // Set our current index to where the signature starts.
                reader.index = at_index;

                reader.is_v_5_0 = buffer[reader.index..reader.index + GENERAL_DIR_SIG_5_0.len()] == GENERAL_DIR_SIG_5_0;

                debug!("Signature Index: {at_index}, is 5.0 = {}", reader.is_v_5_0);

                // TODO: Remove.
                if reader.index + GENERAL_DIR_SIZE_KNOWN >= buffer.len() {
                    reader.seek_to_index(&mut buffer).await?;
                }

                // Double check.
                if reader.is_v_5_0 {
                    assert_eq!(&buffer[reader.index..reader.index + GENERAL_DIR_SIG_5_0.len()], &GENERAL_DIR_SIG_5_0);
                    reader.skip::<8>();

                    // General archive layout
                    //     Self-extracting module (optional)
                    //     RAR 5.0 signature
                    //     Archive encryption header (optional)
                    //     Main archive header
                    //     Archive comment service header (optional)
                    //
                    //     File header #
                    //     Service headers (NTFS ACL, streams, etc.) for preceding file (optional).
                    //     ...
                    //
                    //     Recovery record (optional).
                    //     End of archive header.

                    // Iterate through headers.
                    loop {
                        let general_header = GeneralHeader::parse(&mut reader, &mut buffer).await?;

                        if !reader.is_v_5_0 {
                            debug!("{general_header:#?}");
                        }

                        match general_header.type_of {
                            HeaderType::MainArchive => {
                                let header = MainArchiveHeader::parse(
                                    general_header,
                                    &mut reader,
                                    &mut buffer
                                ).await?;

                                debug!("{header:#?}");

                                main_archive = Some(header);
                            }

                            HeaderType::File => {
                                let header = FileArchiveHeader::parse(
                                    general_header,
                                    &mut reader,
                                    &mut buffer
                                ).await?;

                                debug!("{header:#?}");

                                files.push(header);
                            }

                            HeaderType::EndOfArchive => {
                                let header = EndOfArchiveHeader::parse(
                                    general_header,
                                    &mut reader,
                                    &mut buffer
                                ).await?;

                                debug!("{header:#?}");

                                end_of_archive = Some(header);

                                break;
                            }

                            v => unimplemented!("{v:?}")
                        }
                    }
                } else {
                    assert_eq!(&buffer[reader.index..reader.index + GENERAL_DIR_SIG_4_0.len()], &GENERAL_DIR_SIG_4_0);
                    reader.skip::<7>();

                    // Iterate through headers.
                    loop {
                        let general_header = GeneralHeader4::parse(&mut reader, &mut buffer).await?;

                        debug!("{general_header:#?}");

                        match general_header.type_of {
                            HeaderType4_0::Archive => {
                                reader.skip::<2>(); // 0 0
                                reader.skip::<4>(); // 0 0 0 0

                                // debug!("{header:#?}");
                            }

                            HeaderType4_0::File => {
                                let pack_size = reader.next_u32(&mut buffer).await?;
                                debug!(pack_size);

                                let unp_size = reader.next_u32(&mut buffer).await?;
                                debug!(unp_size);

                                let host_os = reader.next_u8(&mut buffer).await?;
                                debug!(host_os);

                                let file_crc = reader.next_u32(&mut buffer).await?;
                                debug!(file_crc);

                                let ftime = reader.next_u32(&mut buffer).await?;
                                debug!(ftime);

                                let unp_ver = reader.next_u8(&mut buffer).await?;
                                debug!(unp_ver);

                                let method = reader.next_u8(&mut buffer).await?;
                                debug!(method);

                                let name_size = reader.next_u16(&mut buffer).await?;
                                debug!(name_size);

                                let attr = reader.next_u32(&mut buffer).await?;
                                debug!(attr);

                                // TODO: High 4 bytes of 64-bit value of file size.
                                // TODO: Optional value, presents only if bit 0x100 in HEAD_FLAGS is set.
                                // let high_pack_size = reader.next_u32(&mut buffer).await?;
                                // debug!(high_pack_size);

                                // let high_unp_size = reader.next_u32(&mut buffer).await?;
                                // debug!(high_unp_size);

                                let file_name = String::from_utf8(reader.get_chunk_amount(&mut buffer, 23).await?)?;
                                debug!(file_name);

                                // TODO: present if (HEAD_FLAGS & 0x400) != 0
                                // let salt = reader.next_u64(&mut buffer).await?;
                                // debug!(salt);

                                // TODO: present if (HEAD_FLAGS & 0x1000) != 0
                                // let ext_time = reader.get_chunk_amount(&mut buffer, 23).await?);
                                // debug!(ext_time);

                                // read file header ( first 7 bytes already read )
                                // read or skip HEAD_SIZE-sizeof(FILE_HEAD) bytes
                                // if (HEAD_FLAGS & 0x100)
                                //    read or skip HIGH_PACK_SIZE*0x100000000+PACK_SIZE bytes
                                // else
                                //    read or skip PACK_SIZE bytes

                                let _value_packed = reader.get_chunk_amount(&mut buffer, pack_size as usize).await?;
                                // TODO: It seems like RARs' compression format is confidential.
                                // Look at https://github.com/aawc/unrar/blob/d84d61312db5dd83ed1da9fe3e45cb233a56630c/unpack.cpp#L149
                            }

                            v => unimplemented!("{v:?}")
                        }
                    }
                }

                if reader.index != reader.last_read_amount {
                    debug!("Extra Info after End Of Archive");
                }

                break;
            }

            // Nothing left to read?
            if reader.last_read_amount < buffer.len() {
                break;
            }

            // We negate the signature size to ensure we didn't get a partial previously.
            // We remove 1 from size to prevent (end of buffer) duplicates.
            reader.file.seek(SeekFrom::Current(1 - SIGNATURE_SIZE as i64)).await?;
        }

        if reader.is_v_5_0 {
            Ok(Self::Five {
                main_archive: main_archive.ok_or(Error::MissingMainHeader)?,
                end_of_archive: end_of_archive.ok_or(Error::MissingEndHeader)?,
                file,
                files,
            })
        } else {
            Ok(Self::Four {
                file,
            })
        }
    }
}


pub struct ArchiveReader<'a> {
    // TODO: Utilize BufReader
    file: &'a mut File,

    is_v_5_0: bool,

    index: usize,
    last_read_amount: usize,
}

impl<'a> ArchiveReader<'a> {
    pub async fn init(file: &'a mut File, is_v_5_0: bool) -> Result<ArchiveReader<'a>> {
        // Seek back to start.
        file.seek(SeekFrom::Start(0)).await?;

        Ok(Self {
            file,
            is_v_5_0,

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

    fn find_signature_pos(&self, buffer: &[u8]) -> Option<usize> {
        buffer[self.index..].windows(GENERAL_DIR_SIG_4_0.len())
            .zip(buffer[self.index..].windows(GENERAL_DIR_SIG_5_0.len()))
            .position(|v| v.0 == GENERAL_DIR_SIG_4_0 || v.1 == GENERAL_DIR_SIG_5_0)
            .map(|offset| self.index + offset)
    }

    async fn next_u8(&mut self, buffer: &mut [u8; BUFFER_SIZE]) -> Result<u8> {
        Ok(self.get_next_chunk::<1>(buffer).await?[0])
    }

    async fn next_u16(&mut self, buffer: &mut [u8; BUFFER_SIZE]) -> Result<u16> {
        let buf = self.get_next_chunk::<2>(buffer).await?;

        Ok(bytes_to_u16(buf))
    }

    async fn next_u32(&mut self, buffer: &mut [u8; BUFFER_SIZE]) -> Result<u32> {
        Ok(bytes_to_u32(self.get_next_chunk::<4>(buffer).await?))
    }

    async fn next_u64(&mut self, buffer: &mut [u8; BUFFER_SIZE]) -> Result<u64> {
        Ok(bytes_to_u64(self.get_next_chunk::<8>(buffer).await?))
    }


    /// Can include one or more bytes, where lower 7 bits of every byte contain integer data and highest bit in every byte is the continuation flag.
    ///
    /// If highest bit is 0, this is the last byte in sequence.
    ///
    /// So first byte contains 7 least significant bits of integer and continuation flag.
    ///
    /// Second byte, if present, contains next 7 bits and so on.
    async fn next_vint(&mut self, buffer: &mut [u8; BUFFER_SIZE]) -> Result<u64> {
        // 0, 7, 14, ...
        let mut shift_amount: u64 = 0;
        let mut decoded_value: u64 = 0;

        loop {
            let next_byte = self.get_next_chunk::<1>(buffer).await?[0];

            decoded_value |= ((next_byte & 0b0111_1111) as u64) << shift_amount;

            // See if we're supposed to keep reading
            if (next_byte & 0b1000_0000) != 0 {
                shift_amount += 7;
            } else {
                return Ok(decoded_value);
            }
        }
    }
}

pub(crate) fn bytes_to_u64(bytes: &[u8]) -> u64 {
    assert!(bytes.len() == 8);

    (bytes[7] as u64) << 56 |
    (bytes[6] as u64) << 48 |
    (bytes[5] as u64) << 40 |
    (bytes[4] as u64) << 32 |
    (bytes[3] as u64) << 24 |
    (bytes[2] as u64) << 16 |
    (bytes[1] as u64) << 8 |
    bytes[0] as u64

}

pub(crate) fn bytes_to_u32(bytes: &[u8]) -> u32 {
    assert!(bytes.len() == 4);

    (bytes[3] as u32) << 24 |
    (bytes[2] as u32) << 16 |
    (bytes[1] as u32) << 8 |
    bytes[0] as u32

}

pub(crate) fn bytes_to_u16(bytes: &[u8]) -> u16 {
    assert!(bytes.len() == 2);

    (bytes[1] as u16) << 8 |
    bytes[0] as u16

}

fn extract_vint(buffer: &[u8]) -> (usize, u64) {
    let mut shift_amount: u64 = 0;
    let mut decoded_value: u64 = 0;

    let len = buffer.iter().take_while(|v| is_cont_bit(**v)).count() + 1;

    for &next_byte in &buffer[0..len] {
        decoded_value |= ((next_byte & 0b0111_1111) as u64) << shift_amount;

        // See if we're supposed to keep reading
        if (next_byte & 0b1000_0000) != 0 {
            shift_amount += 7;
        } else {
            break;
        }
    }

    (len, decoded_value)
}

fn is_cont_bit(value: u8) -> bool {
    // 0b1000_0000 -> 0b0000_0001
    value >> 7 == 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collect_vint() {
        assert_eq!(
            extract_vint(&[0x01, 0xFF, 0x00]),
            (1, 0x01)
        );
        assert_eq!(
            extract_vint(&[0xFF, 0x01, 0x00]),
            (2, 0xFF)
        );
        assert_eq!(
            extract_vint(&[0xFF, 0xFF, 0x00]),
            (3, 0x3FFF)
        );
    }

    #[test]
    fn test_is_vint_bit() {
        assert!(is_cont_bit(0b1111_1111));
        assert!(!is_cont_bit(0b0111_1111));
        assert!(!is_cont_bit(0b0000_0000));
        assert!(!is_cont_bit(0b0000_0001));
    }
}