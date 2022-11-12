use bitflags::bitflags;
use num_enum::{IntoPrimitive, TryFromPrimitive};

use crate::{ArchiveReader, Result, BUFFER_SIZE};

/// Signature for 1.5 - 4.0
pub(crate) const GENERAL_DIR_SIG_4_0: [u8; 7] = [0x52, 0x61, 0x72, 0x21, 0x1A, 0x07, 0x00];

// TODO: 0x52 0x45 0x7E 0x5E - Even older signature.

#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum HeaderType4_0 {
    MarkBlock = 0x72,
    Archive,
    File,
    OldComment,
    OldAuthInfo,
    OldSubBlock,
    OldRecoveryRecord,
    OldAuthInfo2,
    SubBlock,
    Terminator,
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
    pub struct ArchiveFlags4_0: u64 {
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

        // 4.0
        const UNKNOWN = 0x9020;
    }
}

#[derive(Debug, Clone, Copy)]
pub enum DictionaryBits {
    Size64,
    Size128,
    Size256,
    Size512,
    Size1024,
    Size2048,
    Size4096,
    Directory,
}

impl DictionaryBits {
    pub fn from_bits(value: u8) -> Self {
        match value {
            0b000 => Self::Size64,
            0b001 => Self::Size128,
            0b010 => Self::Size256,
            0b011 => Self::Size512,
            0b100 => Self::Size1024,
            0b101 => Self::Size2048,
            0b110 => Self::Size4096,
            0b111 => Self::Directory,

            _ => unreachable!(),
        }
    }
}

#[derive(Debug)]
pub struct GeneralHeader4 {
    /// CRC32 of header data starting from Header size field and up to and including the optional extra area.
    pub crc32: u32,

    /// Size of header data starting from Header type field and up to and including the optional extra area.
    ///
    /// This field must not be longer than 3 bytes in current implementation, resulting in 2 MB maximum header size.
    pub size: u64,

    pub type_of: HeaderType4_0,

    pub flags: HeaderFlags,

    /// Optional field, present only if 0x0001 header flag is set.
    pub extra_area_size: u64, // TODO: Option

    /// Optional field, present only if 0x0002 header flag is set.
    pub data_size: u64, // TODO: Option

    pub dictionary: Option<DictionaryBits>,
}

impl GeneralHeader4 {
    pub async fn parse(
        reader: &mut ArchiveReader<'_>,
        buffer: &mut [u8; BUFFER_SIZE],
    ) -> Result<Self> {
        println!("crc: {:X?}", &buffer[reader.index..reader.index + 2]);
        let crc32 = reader.next_u16(buffer).await? as u32;

        // HeaderType4_0::try_from(reader.next_u8(buffer).await?)?
        println!("type: {:X?}", &buffer[reader.index..reader.index + 1]);
        let type_of = HeaderType4_0::try_from(reader.next_u8(buffer).await?)?;

        let (flags, dictionary) = {
            println!("flags: {:X?}", &buffer[reader.index..reader.index + 2]);
            let mut value = reader.next_u16(buffer).await? as u64;

            let dictionary = if type_of == HeaderType4_0::File {
                value &= !0b1110_0000;
                Some(DictionaryBits::from_bits(((value >> 5) & 0b0111) as u8))
            } else {
                None
            };

            let flags = HeaderFlags::from_bits(value).ok_or(crate::Error::InvalidBitFlag {
                name: "Header 4",
                flag: value,
            })?;

            (flags, dictionary)
        };

        println!("size: {:X?}", &buffer[reader.index..reader.index + 2]);
        let size = reader.next_u16(buffer).await? as u64;

        // 0x4000 = 16384
        // 0x8000 = 32768
        // The field ADD_SIZE present only if (HEAD_FLAGS & 0x8000) != 0.
        // Total block size is HEAD_SIZE if (HEAD_FLAGS & 0x8000) == 0 and HEAD_SIZE+ADD_SIZE if the field ADD_SIZE is present - when (HEAD_FLAGS & 0x8000) != 0.
        //
        // In each block the followings bits in HEAD_FLAGS have the same meaning:
        //     0x4000 - if set, older RAR versions will ignore the block and remove it when the archive is updated. If clear, the block is copied to the new archive file when the archive is updated;
        //     0x8000 - if set, ADD_SIZE field is present and the full block size is HEAD_SIZE+ADD_SIZE.

        let add_size = if flags.contains(HeaderFlags::EXTRA_AREA) {
            println!(
                "extra_area_size: {:X?}",
                &buffer[reader.index..reader.index + 8]
            );
            reader.next_u64(buffer).await?
        } else {
            0
        };

        Ok(Self {
            crc32,
            size,
            type_of,
            flags,
            dictionary,
            extra_area_size: add_size,
            data_size: 0,
        })
    }
}
