use std::io::SeekFrom;

use tokio::io::{AsyncSeekExt, AsyncReadExt};

use crate::{Archive, BUFFER_SIZE, SIGNATURE_SIZE};


pub(crate) const END_CENTRAL_DIR_SIG: [u8; 4] = [0x50, 0x4B, 0x05, 0x06];
pub(crate) const END_CENTRAL_DIR_SIZE_KNOWN: usize = 22;



#[derive(Debug, Clone)]
pub struct ArchiveInfo {
    /// Total amount of files and folders
    records: u16,
    /// Size of archive.
    size: u32,
    /// Archive Comment, if there is one.
    comment: String,
}



/// Is at the end of every Zip file
#[derive(Debug)]
pub struct EndCentralDirHeader {
    // Number of this disk (or 0xffff for ZIP64)
    current_disk_number: u16,
    // Disk where central directory starts (or 0xffff for ZIP64)
    start_disk_number: u16,
    // Number of central directory records on this disk (or 0xffff for ZIP64)
    record_count_on_curr_disk: u16,
    // Total number of central directory records (or 0xffff for ZIP64)
    total_record_count: u16,
    // Size of central directory (bytes) (or 0xffffffff for ZIP64)
    size_of: u32,
    // Offset of start of central directory, relative to start of archive (or 0xffffffff for ZIP64)
    curr_offset: u32,
    // Comment length (n)
    comment_len: u16,
    // Comment
    comment: String,
}

impl EndCentralDirHeader {
    pub async fn parse(archive: &mut Archive, buffer: &mut [u8; BUFFER_SIZE]) -> Self {
        assert_eq!(&buffer[archive.index..archive.index + 4], &END_CENTRAL_DIR_SIG);

        archive.skip::<4>();

        let mut header = EndCentralDirHeader {
            current_disk_number: archive.next_u16(buffer).await,
            start_disk_number: archive.next_u16(buffer).await,
            record_count_on_curr_disk: archive.next_u16(buffer).await,
            total_record_count: archive.next_u16(buffer).await,
            size_of: archive.next_u32(buffer).await,
            curr_offset: archive.next_u32(buffer).await,
            comment_len: archive.next_u16(buffer).await,
            comment: String::new(),
        };

        header.comment = String::from_utf8(archive.get_chunk_amount(buffer, header.comment_len as usize).await).unwrap();

        header
    }

    pub async fn find(archive: &mut Archive) -> EndCentralDirHeader {
        let mut buffer = [0u8; BUFFER_SIZE];

        // Reset back to start.
        archive.file.seek(SeekFrom::Start(0)).await.unwrap();


        loop {
            // Read updates seek position
            archive.last_read_amount = archive.file.read(&mut buffer).await.unwrap();
            archive.index = 0;

            if let Some(at_index) = archive.find_next_signature(&buffer, END_CENTRAL_DIR_SIG) {
                // Set our current index to where the signature starts.
                archive.index = at_index;

                // println!("Found End Header @ {} {} {:x?}", archive.file.stream_position().unwrap() as usize + archive.index, archive.index, &buffer[archive.index..archive.index + 4]);

                assert_eq!(&buffer[archive.index..archive.index + 4], &END_CENTRAL_DIR_SIG);

                // TODO: Remove.
                if archive.index + END_CENTRAL_DIR_SIZE_KNOWN as usize >= buffer.len() {
                    archive.seek_to_index(&mut buffer).await;
                }

                let header = Self::parse(archive, &mut buffer).await;

                // println!("{header:#?}");

                return header;
            }

            // Nothing left to read?
            if archive.last_read_amount < buffer.len() {
                break;
            }

            // We negate the signature size to ensure we didn't get a partial previously. We remove 1 from size to prevent (end of buffer) duplicates.
            archive.file.seek(SeekFrom::Current(1 - SIGNATURE_SIZE as i64)).await.unwrap();
        }

        panic!("Missing End Header");
    }
}

impl From<&EndCentralDirHeader> for ArchiveInfo {
    fn from(value: &EndCentralDirHeader) -> Self {
        Self {
            comment: value.comment.clone(),
            size: value.size_of,
            records: value.total_record_count,
        }
    }
}