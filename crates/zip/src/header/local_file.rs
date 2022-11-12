use tokio::io::AsyncReadExt;

use crate::{ArchiveReader, CompressionType, Result, BUFFER_SIZE};

pub(crate) const LOCAL_FILE_HEADER_SIG: [u8; 4] = [0x50, 0x4B, 0x03, 0x04];

#[derive(Debug)]
pub struct LocalFileHeader {
    // Version needed to extract (minimum)
    min_version: u16,
    // General purpose bit flag
    gp_flag: u16,
    // Compression method; e.g. none = 0, DEFLATE = 8 (or "\0x08\0x00")
    compression: CompressionType,
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
    file_name: String,
    // Extra field
    extra_field: Vec<(u16, u16)>,
}

impl LocalFileHeader {
    pub async fn parse(
        reader: &mut ArchiveReader<'_>,
        start_offset: u64,
    ) -> Result<(Self, String)> {
        let mut buffer = [0u8; BUFFER_SIZE];

        reader.seek_to(start_offset).await?;
        reader.last_read_amount = reader.file.read(&mut buffer).await?;

        assert_eq!(
            &buffer[reader.index..reader.index + 4],
            &LOCAL_FILE_HEADER_SIG
        );

        reader.skip::<4>();

        let mut header = LocalFileHeader {
            min_version: reader.next_u16(&mut buffer).await?,
            gp_flag: reader.next_u16(&mut buffer).await?,
            compression: CompressionType::try_from(reader.next_u16(&mut buffer).await?)?,
            file_last_mod_time: reader.next_u16(&mut buffer).await?,
            file_last_mod_date: reader.next_u16(&mut buffer).await?,
            crc_32: reader.next_u32(&mut buffer).await?,
            compressed_size: reader.next_u32(&mut buffer).await?,
            uncompressed_size: reader.next_u32(&mut buffer).await?,
            file_name_length: reader.next_u16(&mut buffer).await?,
            extra_field_length: reader.next_u16(&mut buffer).await?,
            file_name: String::new(),
            extra_field: Vec::new(),
        };

        header.file_name = String::from_utf8(
            reader
                .get_chunk_amount(&mut buffer, header.file_name_length as usize)
                .await?,
        )?;
        header.extra_field = reader
            .get_chunk_amount(&mut buffer, header.extra_field_length as usize)
            .await?
            .into_iter()
            .array_chunks::<4>()
            .map(|v| {
                (
                    (u16::from(v[0]) << 8) | u16::from(v[1]),
                    (u16::from(v[2]) << 8) | u16::from(v[3]),
                )
            })
            .collect();

        let comp_contents = reader
            .get_chunk_amount(&mut buffer, header.compressed_size as usize)
            .await?;
        let contents = header.compression.decompress(comp_contents)?;

        // TODO: Determine what we want to do with the Header. It's just a shrunken form of Central Directory File Header.

        Ok((header, contents))
    }
}
