use bitflags::bitflags;

use crate::{ArchiveReader, Result, BUFFER_SIZE};

use super::GeneralHeader;

#[derive(Debug)]
pub struct EndOfArchiveHeader {
    pub general_header: GeneralHeader,

    pub archive_flags: EndOfArchiveFlags,
}

impl EndOfArchiveHeader {
    pub async fn parse(
        general_header: GeneralHeader,
        reader: &mut ArchiveReader<'_>,
        buffer: &mut [u8; BUFFER_SIZE],
    ) -> Result<Self> {
        let archive_flags = {
            let value = reader.next_vint(buffer).await?;
            EndOfArchiveFlags::from_bits(value).ok_or(crate::Error::InvalidBitFlag {
                name: "End Of Archive",
                flag: value,
            })?
        };

        Ok(Self {
            general_header,
            archive_flags,
        })
    }
}

bitflags! {
    /// 0x0001  Archive is volume and it is not last volume in the set
    pub struct EndOfArchiveFlags: u64 {
        /// Archive is volume and it is not last volume in the set
        const VOLUME = 0b0000_0001;
    }
}
