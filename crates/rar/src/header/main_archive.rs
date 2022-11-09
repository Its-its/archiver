//! Header File

use crate::{BUFFER_SIZE, ArchiveReader, Result};

use super::{GeneralHeader, ArchiveFlags};

#[derive(Debug)]
pub(crate) struct MainArchiveHeader {
    pub general_header: GeneralHeader,

    pub archive_flags: ArchiveFlags,

    /// Optional field, present only if 0x0002 archive flag is set. Not present for first volume, 1 for second volume, 2 for third and so on.
    pub volume_number: Option<u64>,

    /// Optional area containing additional header fields, present only if 0x0001 header flag is set.
    pub extra_area: Option<Vec<u8>>,
}

impl MainArchiveHeader {
    pub async fn parse(
        general_header: GeneralHeader,
        reader: &mut ArchiveReader<'_>,
        buffer: &mut [u8; BUFFER_SIZE],
    ) -> Result<Self> {
        let archive_flags = ArchiveFlags::from_bits(reader.next_vint(buffer).await?).expect("Archive Flag");

        let volume_number = if archive_flags.contains(ArchiveFlags::VOLUME_NUMBER) {
            Some(reader.next_vint(buffer).await?)
        } else {
            None
        };

        let extra_area = if general_header.extra_area_size != 0 {
            Some(reader.get_chunk_amount(buffer, general_header.extra_area_size as usize).await?)
        } else {
            None
        };

        let mut header = Self {
            general_header,
            archive_flags,
            volume_number,
            extra_area,
        };

        Ok(header)
    }
}