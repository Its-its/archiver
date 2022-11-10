use bitflags::bitflags;

use crate::{BUFFER_SIZE, ArchiveReader, Result};

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
        let archive_flags = EndOfArchiveFlags::from_bits(reader.next_vint(buffer).await?).expect("Archive Flag");

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