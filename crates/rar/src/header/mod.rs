mod archive_comment_service;
mod archive_encryption;
mod file;
mod end_of_archive;
mod main_archive;
mod recovery;
mod service;

use std::io::SeekFrom;

use bitflags::bitflags;
pub use main_archive::*;

use num_enum::{TryFromPrimitive, IntoPrimitive};
use tokio::io::{AsyncReadExt, AsyncSeekExt};

use crate::{ArchiveReader, BUFFER_SIZE, Result, Error};


/// Signature takes up 4 bytes.
const SIGNATURE_SIZE: usize = 4;

pub(crate) const GENERAL_DIR_SIG_5_0: [u8; 8] = [0x52 , 0x61 , 0x72 , 0x21 , 0x1A , 0x07 , 0x01 , 0x00];
pub(crate) const GENERAL_DIR_SIZE_KNOWN: usize = 12;


// 5.0 + 52 61 72 21 1A 07 01 00
// 1.5 + 52 61 72 21 1A 07 00


/// Type of archive header. Possible values are:
///
///   1   Main archive header.
///
///   2   File header.
///
///   3   Service header.
///
///   4   Archive encryption header.
///
///   5   End of archive header.
#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum HeaderType {
    MainArchive = 1,
    File,
    Service,
    ArchiveEncryption,
    EndOfArchive,
}


bitflags! {
    /// 0x0001  Volume. Archive is a part of multivolume set.
    ///
    /// 0x0002  Volume number field is present. This flag is present in all volumes except first.
    ///
    /// 0x0004  Solid archive.
    ///
    /// 0x0008  Recovery record is present.
    ///
    /// 0x0010  Locked archive.
    pub struct ArchiveFlags: u64 {
        /// Volume. Archive is a part of multivolume set.
        const VOLUME = 0b0000_0001;
        /// Volume number field is present. This flag is present in all volumes except first.
        const VOLUME_NUMBER = 0b0000_0010;
        /// Solid archive.
        const SOLID = 0b0000_0100;
        /// Recovery record is present.
        const RECOVERY = 0b0000_1000;
        /// Locked archive.
        const LOCKED = 0b0001_0000;
    }
}


bitflags! {
    /// Flags common for all headers:
    ///
    ///   0x0001   Extra area is present in the end of header.
    ///
    ///   0x0002   Data area is present in the end of header.
    ///
    ///   0x0004   Blocks with unknown type and this flag must be skipped when updating an archive.
    ///
    ///   0x0008   Data area is continuing from previous volume.
    ///
    ///   0x0010   Data area is continuing in next volume.
    ///
    ///   0x0020   Block depends on preceding file block.
    ///
    ///   0x0040   Preserve a child block if host block is modified.
    pub struct HeaderFlags: u64 {
        /// Extra area is present in the end of header.
        const EXTRA_AREA = 0b0000_0001;
        /// Data area is present in the end of header.
        const DATA_AREA = 0b0000_0010;
        /// Blocks with unknown type and this flag must be skipped when updating an archive.
        const SKIP = 0b0000_0100;
        /// Data area is continuing from previous volume.
        const DATA_PREV = 0b0000_1000;
        /// Data area is continuing in next volume.
        const DATA_NEXT = 0b0001_0000;
        /// Block depends on preceding file block.
        const PRECEDING = 0b0010_0000;
        /// Preserve a child block if host block is modified.
        const PRESERVE = 0b0100_0000;
    }
}



pub(crate) async fn find_signature_header(reader: &mut ArchiveReader<'_>) -> Result<()> {
    let mut buffer = [0u8; BUFFER_SIZE];

    // Reset back to start.
    reader.seek_to(0).await?;

    loop {
        // Read updates seek position
        reader.last_read_amount = reader.file.read(&mut buffer).await?;
        reader.index = 0;

        if let Some(at_index) = reader.find_next_signature(&buffer, GENERAL_DIR_SIG_5_0) {
            // Set our current index to where the signature starts.
            reader.index = at_index;

            assert_eq!(&buffer[reader.index..reader.index + GENERAL_DIR_SIG_5_0.len()], &GENERAL_DIR_SIG_5_0);

            // TODO: Remove.
            if reader.index + GENERAL_DIR_SIZE_KNOWN >= buffer.len() {
                reader.seek_to_index(&mut buffer).await?;
            }

            // Double check.
            assert_eq!(&buffer[reader.index..reader.index + GENERAL_DIR_SIG_5_0.len()], &GENERAL_DIR_SIG_5_0);

            reader.skip::<8>();

            for _ in 0..2 {
                println!("=============================================");

                let general_header = GeneralHeader::parse(reader, &mut buffer).await?;

                match general_header.type_of {
                    HeaderType::MainArchive => {
                        let header = MainArchiveHeader::parse(
                            general_header,
                            reader,
                            &mut buffer
                        ).await?;

                        println!("{header:#?}");
                    }

                    // HeaderType::File => {
                    //     //
                    // }

                    v => unimplemented!("{v:?}")
                }
            }

            return Ok(());
        }

        // Nothing left to read?
        if reader.last_read_amount < buffer.len() {
            break;
        }

        // We negate the signature size to ensure we didn't get a partial previously. We remove 1 from size to prevent (end of buffer) duplicates.
        reader.file.seek(SeekFrom::Current(1 - SIGNATURE_SIZE as i64)).await?;
    }

    Err(Error::MissingHeader)
}



#[derive(Debug)]
pub(crate) struct GeneralHeader {
    /// CRC32 of header data starting from Header size field and up to and including the optional extra area.
    pub crc32: u32,

    /// Size of header data starting from Header type field and up to and including the optional extra area.
    ///
    /// This field must not be longer than 3 bytes in current implementation, resulting in 2 MB maximum header size.
    pub size: u64,

    pub type_of: HeaderType,

    pub flags: HeaderFlags,

    /// Optional field, present only if 0x0001 header flag is set.
    pub extra_area_size: u64,

    /// Optional field, present only if 0x0002 header flag is set.
    pub data_size: u64,
}

impl GeneralHeader {
    pub async fn parse(reader: &mut ArchiveReader<'_>, buffer: &mut [u8; BUFFER_SIZE]) -> Result<Self> {
        let crc32 = reader.next_u32(buffer).await?;
        let size = reader.next_vint(buffer).await?;
        let type_of = HeaderType::try_from(reader.next_vint(buffer).await? as u8)?;
        let flags = HeaderFlags::from_bits(reader.next_vint(buffer).await?).expect("Header Flag");

        Ok(Self {
            crc32,
            size,
            type_of,
            flags,
            extra_area_size: if flags.contains(HeaderFlags::EXTRA_AREA) {
                reader.next_vint(buffer).await?
            } else {
                0
            },
            data_size: if flags.contains(HeaderFlags::DATA_AREA) {
                reader.next_vint(buffer).await?
            } else {
                0
            },
        })
    }
}