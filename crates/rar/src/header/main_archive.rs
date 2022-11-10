//! Main Archive

use crate::{BUFFER_SIZE, ArchiveReader, Result};

use super::{GeneralHeader, ArchiveFlags, HeaderFlags};

#[derive(Debug)]
pub struct MainArchiveHeader {
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
        let archive_flags = {
            let value = reader.next_vint(buffer).await?;
            ArchiveFlags::from_bits(value)
            .ok_or(crate::Error::InvalidBitFlag { name: "Archive", flag: value })?
        };

        let volume_number = if archive_flags.contains(ArchiveFlags::VOLUME_NUMBER) {
            Some(reader.next_vint(buffer).await?)
        } else {
            None
        };

        let extra_area = if general_header.flags.contains(HeaderFlags::EXTRA_AREA) {
            Some(reader.get_chunk_amount(buffer, general_header.extra_area_size as usize).await?)
        } else {
            None
        };

        Ok(Self {
            general_header,
            archive_flags,
            volume_number,
            extra_area,
        })
    }
}


// TODO: Extra Header
// Type	Name	Description
// 0x01	Locator	Contains positions of different service blocks, so they can be accessed quickly, without scanning the entire archive. This record is optional. If it is missing, it is still necessary to scan the entire archive to verify presence of service blocks.
// 0x02	Metadata	Optional record storing archive metadata, which includes archive original name and time.