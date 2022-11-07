//! The EOCD file. Every Zip file has to have one at the end for it to be valid.

use std::io::SeekFrom;

use tokio::io::{AsyncSeekExt, AsyncReadExt};

use crate::{BUFFER_SIZE, SIGNATURE_SIZE, ArchiveReader, Result, Error};


pub(crate) const END_CENTRAL_DIR_SIG: [u8; 4] = [0x50, 0x4B, 0x05, 0x06];
pub(crate) const END_CENTRAL_DIR_SIZE_KNOWN: usize = 22;



/// Used to share the relevant Zip Info.
#[derive(Debug, Clone)]
pub struct ArchiveInfo {
    /// Does the zip use multiple disks
    pub is_multi_disk: bool,
    /// Total amount of files and folders
    pub records: u16,
    /// Size of archive.
    pub size: u32,
    /// Archive Comment, if there is one.
    pub comment: String,
}



/// Is at the end of every Zip file
#[derive(Debug, Default)]
pub(crate) struct EndCentralDirHeader {
    // Number of this disk (or 0xffff for ZIP64)
    pub current_disk_number: u16,
    // Disk where central directory starts (or 0xffff for ZIP64)
    pub start_disk_number: u16,
    // Number of central directory records on this disk (or 0xffff for ZIP64)
    pub record_count_on_curr_disk: u16,
    // Total number of central directory records (or 0xffff for ZIP64)
    pub total_record_count: u16,
    // Size of central directory (bytes) (or 0xffffffff for ZIP64)
    pub size_of: u32,
    // Offset of start of central directory, relative to start of archive (or 0xffffffff for ZIP64)
    pub curr_offset: u32,
    // Comment length (n)
    pub comment_len: u16,
    // Comment
    pub comment: String,
}

impl EndCentralDirHeader {
    pub async fn parse(reader: &mut ArchiveReader<'_>, buffer: &mut [u8; BUFFER_SIZE]) -> Result<Self> {
        assert_eq!(&buffer[reader.index..reader.index + 4], &END_CENTRAL_DIR_SIG);

        reader.skip::<4>();

        let mut header = EndCentralDirHeader {
            current_disk_number: reader.next_u16(buffer).await?,
            start_disk_number: reader.next_u16(buffer).await?,
            record_count_on_curr_disk: reader.next_u16(buffer).await?,
            total_record_count: reader.next_u16(buffer).await?,
            size_of: reader.next_u32(buffer).await?,
            curr_offset: reader.next_u32(buffer).await?,
            comment_len: reader.next_u16(buffer).await?,
            comment: String::new(),
        };

        header.comment = String::from_utf8(reader.get_chunk_amount(buffer, header.comment_len as usize).await?)?;

        Ok(header)
    }

    pub async fn find(reader: &mut ArchiveReader<'_>) -> Result<EndCentralDirHeader> {
        let mut buffer = [0u8; BUFFER_SIZE];

        // Reset back to start.
        reader.seek_to(0).await?;

        loop {
            // Read updates seek position
            reader.last_read_amount = reader.file.read(&mut buffer).await?;
            reader.index = 0;

            if let Some(at_index) = reader.find_next_signature(&buffer, END_CENTRAL_DIR_SIG) {
                // Set our current index to where the signature starts.
                reader.index = at_index;

                // println!("Found End Header @ {} {} {:x?}", archive.file.stream_position().unwrap() as usize + archive.index, archive.index, &buffer[archive.index..archive.index + 4]);

                assert_eq!(&buffer[reader.index..reader.index + 4], &END_CENTRAL_DIR_SIG);

                // TODO: Remove.
                if reader.index + END_CENTRAL_DIR_SIZE_KNOWN as usize >= buffer.len() {
                    reader.seek_to_index(&mut buffer).await?;
                }

                let header = Self::parse(reader, &mut buffer).await?;

                // println!("{header:#?}");

                return Ok(header);
            }

            // Nothing left to read?
            if reader.last_read_amount < buffer.len() {
                break;
            }

            // We negate the signature size to ensure we didn't get a partial previously. We remove 1 from size to prevent (end of buffer) duplicates.
            reader.file.seek(SeekFrom::Current(1 - SIGNATURE_SIZE as i64)).await?;
        }

        Err(Error::MissingEndHeader)
    }
}

impl From<&EndCentralDirHeader> for ArchiveInfo {
    fn from(value: &EndCentralDirHeader) -> Self {
        Self {
            is_multi_disk: value.start_disk_number != value.current_disk_number,
            comment: value.comment.clone(),
            size: value.size_of,
            records: value.total_record_count,
        }
    }
}