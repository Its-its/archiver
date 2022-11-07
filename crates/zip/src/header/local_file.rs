use std::io::SeekFrom;

use tokio::io::{AsyncSeekExt, AsyncReadExt};

use crate::{Archive, BUFFER_SIZE};



pub(crate) const LOCAL_FILE_HEADER_SIG: [u8; 4] = [0x50, 0x4B, 0x03, 0x04];


#[derive(Debug)]
pub struct LocalFileHeader {
    // Version needed to extract (minimum)
    min_version: u16,
    // General purpose bit flag
    gp_flag: u16,
    // Compression method; e.g. none = 0, DEFLATE = 8 (or "\0x08\0x00")
    compression: u16,
    // File last modification time
    file_last_mod_time: u16,
    // File last modification date
    file_last_mod_date: u16,
    // CRC-32 of uncompressed data
    crc_32: u32,
    // Compressed size (or 0xffffffff for ZIP64)
    compressed_size: u32,
    // Uncompressed size (or 0xffffffff for ZIP64)
    uncompressed_size: u32,
    // File name length (n)
    file_name_length: u16,
    // Extra field length (m)
    extra_field_length: u16,
    // File name
    pub file_name: String,
    // Extra field
    pub extra_field: Vec<u8>,
}

impl LocalFileHeader {
    pub async fn parse(archive: &mut Archive, start_offset: u64) {
        let mut buffer = [0u8; BUFFER_SIZE];

        archive.file.seek(SeekFrom::Start(start_offset)).await.unwrap();
        archive.last_read_amount = archive.file.read(&mut buffer).await.unwrap();
        archive.index = 0;

        assert_eq!(&buffer[archive.index..archive.index + 4], &LOCAL_FILE_HEADER_SIG);

        archive.skip::<4>();

        let mut header = LocalFileHeader {
            min_version: archive.next_u16(&mut buffer).await,
            gp_flag: archive.next_u16(&mut buffer).await,
            compression: archive.next_u16(&mut buffer).await,
            file_last_mod_time: archive.next_u16(&mut buffer).await,
            file_last_mod_date: archive.next_u16(&mut buffer).await,
            crc_32: archive.next_u32(&mut buffer).await,
            compressed_size: archive.next_u32(&mut buffer).await,
            uncompressed_size: archive.next_u32(&mut buffer).await,
            file_name_length: archive.next_u16(&mut buffer).await,
            extra_field_length: archive.next_u16(&mut buffer).await,
            file_name: String::new(),
            extra_field: Vec::new(),
        };

        header.file_name = String::from_utf8(archive.get_chunk_amount(&mut buffer, header.file_name_length as usize).await).unwrap();
        header.extra_field = archive.get_chunk_amount(&mut buffer, header.extra_field_length as usize).await;

        let contents = String::from_utf8(archive.get_chunk_amount(&mut buffer, header.compressed_size as usize).await).unwrap();

        println!("{header:#?}");
        println!("{contents}");
    }
}