//! File Archive

use bitflags::bitflags;

use crate::{BUFFER_SIZE, ArchiveReader, Result};

use super::{GeneralHeader, HeaderFlags};



bitflags! {
    /// Flags specific for these header types:
    ///
    ///     0x0001  Directory file system object (file header only).
    ///
    ///     0x0002  Time field in Unix format is present.
    ///
    ///     0x0004  CRC32 field is present.
    ///
    ///     0x0008  Unpacked size is unknown.
    ///
    /// If flag 0x0008 is set, unpacked size field is still present, but must be ignored and extraction must be performed until reaching the end of compression stream.
    ///
    /// This flag can be set if actual file size is larger than reported by OS or if file size is unknown such as for all volumes except last when archiving from stdin to multivolume archive.
    pub struct FileFlags: u64 {
        /// Directory file system object (file header only).
        const DIR_FILE_SYS_OBJ = 0b0000_0001;
        /// Time field in Unix format is present.
        const UNIX_TIME = 0b0000_0010;
        /// CRC32 field is present.
        const CRC32_PRESENT = 0b0000_0100;
        /// Unpacked size is unknown.
        const UNPACK_SIZE_UNKNOWN = 0b0000_1000;
    }

    /// Type of operating system used to create the archive.
    ///     0x0000  Windows.
    ///     0x0001  Unix.
    pub struct OperatingSystem: u64 {
        const WINDOWS = 0b0000_0001;
        const UNIX = 0b0000_0010;
    }
}



#[derive(Debug)]
pub struct FileArchiveHeader {
    pub general_header: GeneralHeader,

    pub file_flags: FileFlags,

    /// Unpacked file or service data size.
    pub unpacked_size: u64,

    /// Operating system specific file attributes in case of file header.
    ///
    /// Might be either used for data specific needs or just reserved and set to 0 for service header.
    pub attributes: u64,

    /// File modification time in Unix time format.
    ///
    /// Optional, present if 0x0002 file flag is set.
    pub mtime: Option<u32>,

    /// CRC32 of unpacked file or service data.
    ///
    /// For files split between volumes it contains CRC32 of file packed data contained in current volume for all file parts except the last.
    ///
    /// Optional, present if 0x0004 file flag is set.
    pub data_crc32: Option<u32>,

    /// Lower 6 bits (0x003f mask) contain the version of compression algorithm, resulting in possible 0 - 63 values. Current version is 0.
    ///
    /// 7th bit (0x0040) defines the solid flag. If it is set, RAR continues to use the compression dictionary left after processing preceding files.
    /// It can be set only for file headers and is never set for service headers.
    ///
    /// Bits 8 - 10 (0x0380 mask) define the compression method. Currently only values 0 - 5 are used. 0 means no compression.
    ///
    /// Bits 11 - 14 (0x3c00) define the minimum size of dictionary size required to extract data.
    /// Value 0 means 128 KB, 1 - 256 KB, ..., 14 - 2048 MB, 15 - 4096 MB.
    pub compression_info: FileCompressionInfo,

    pub host_os: OperatingSystem,

    /// File or service header name length.
    pub name_length: u64,

    /// Variable length field containing Name length bytes in UTF-8 format without trailing zero.
    ///
    /// For file header this is a name of archived file.
    /// Forward slash character is used as the path separator both for Unix and Windows names.
    /// Backslashes are treated as a part of name for Unix names and as invalid character for Windows file names.
    /// Type of name is defined by Host OS field.
    ///
    /// If Unix file name contains any high ASCII characters which cannot be correctly converted to Unicode and UTF-8,
    /// we map such characters to to 0xE080 - 0xE0FF private use Unicode area and insert 0xFFFE Unicode non-character to resulting string to indicate that it contains mapped characters,
    /// which need to be converted back when extracting.
    /// Concrete position of 0xFFFE is not defined, we need to search the entire string for it.
    /// Such mapped names are not portable and can be correctly unpacked only on the same system where they were created.
    ///
    /// For service header this field contains a name of service header. Now the following names are used:
    ///     CMT  Archive comment
    ///     QO   Archive quick open data
    ///     ACL  NTFS file permissions
    ///     STM  NTFS alternate data stream
    ///     RR   Recovery record
    pub name: String,

    /// Optional area containing additional header fields, present only if 0x0001 header flag is set.
    pub extra_area: Option<Vec<u8>>,

    /// Optional data area, present only if 0x0002 header flag is set.
    ///
    /// Store file data in case of file header or service data for service header.
    ///
    /// Depending on the compression method value in Compression information can be either uncompressed (compression method 0) or compressed.
    ///
    /// We store the position of the area for referencing later.
    pub data_area: Option<u64>,
}

impl FileArchiveHeader {
    pub async fn parse(
        general_header: GeneralHeader,
        reader: &mut ArchiveReader<'_>,
        buffer: &mut [u8; BUFFER_SIZE],
    ) -> Result<Self> {
        let file_flags = {
            let value = reader.next_vint(buffer).await?;
            FileFlags::from_bits(value)
            .ok_or(crate::Error::InvalidBitFlag { name: "File", flag: value })?
        };

        // TODO: If flag 0x0008 is set, unpacked size field is still present, but must be ignored and extraction must be performed until reaching the end of compression stream.
        // TODO: This flag can be set if actual file size is larger than reported by OS or if file size is unknown such as for all volumes except last when archiving from stdin to multivolume archive.
        let unpacked_size = reader.next_vint(buffer).await?;

        let attributes = reader.next_vint(buffer).await?;

        let mtime = if file_flags.contains(FileFlags::UNIX_TIME) {
            Some(reader.next_u32(buffer).await?)
        } else {
            None
        };

        let data_crc32 = if file_flags.contains(FileFlags::CRC32_PRESENT) {
            Some(reader.next_u32(buffer).await?)
        } else {
            None
        };

        let comp_info = reader.next_vint(buffer).await?;

        let host_os = {
            let value = reader.next_vint(buffer).await?;
            OperatingSystem::from_bits(value)
            .ok_or(crate::Error::InvalidBitFlag { name: "Operating System", flag: value })?
        };

        let name_length = reader.next_vint(buffer).await?;

        let name = String::from_utf8(reader.get_chunk_amount(buffer, name_length as usize).await?.to_vec())?;

        let extra_area = if general_header.flags.contains(HeaderFlags::EXTRA_AREA) {
            Some(reader.get_chunk_amount(buffer, general_header.extra_area_size as usize).await?)
        } else {
            None
        };

        let data_area = if general_header.flags.contains(HeaderFlags::DATA_AREA) {
            let data_pos = reader.get_seek_position().await?;

            reader.index += general_header.data_size as usize;

            if reader.index > buffer.len() {
                reader.seek_to_index(buffer).await?;
            }

            Some(data_pos)
        } else {
            None
        };

        Ok(Self {
            general_header,
            file_flags,
            unpacked_size,
            attributes,
            mtime,
            data_crc32,
            compression_info: FileCompressionInfo::try_from(comp_info)?,
            host_os,
            name_length,
            name,
            extra_area,
            data_area,
        })
    }

    pub async fn read(&self, reader: &mut ArchiveReader<'_>, buffer: &mut [u8; BUFFER_SIZE]) -> Result<String> {
        if let Some(pos) = self.data_area {
            reader.seek_to(pos).await?;

            Ok(String::from_utf8(reader.get_chunk_amount(buffer, self.general_header.data_size as usize).await?)?)
        } else {
            Ok(String::new())
        }
    }
}

#[derive(Debug)]
pub struct FileCompressionInfo {
    /// Lower 6 bits (0x003f mask) contain the version of compression algorithm, resulting in possible 0 - 63 values. Current version is 0.
    ///
    /// 7th bit (0x0040) defines the solid flag. If it is set, RAR continues to use the compression dictionary left after processing preceding files.
    /// It can be set only for file headers and is never set for service headers.
    ///
    /// Bits 8 - 10 (0x0380 mask) define the compression method. Currently only values 0 - 5 are used. 0 means no compression.
    ///
    /// Bits 11 - 14 (0x3c00) define the minimum size of dictionary size required to extract data.
    ///
    /// Value 0 means 128 KB,
    ///     1 - 256 KB,
    ///     ...,
    ///     14 - 2048 MB,
    ///     15 - 4096 MB.
    pub value: u64,
}

impl TryFrom<u64> for FileCompressionInfo {
    type Error = crate::Error;

    fn try_from(value: u64) -> Result<Self, Self::Error> {
        Ok(Self {
            value,
        })
    }
}

// TODO: Extra Area Record
// Type  Name             Description
// 0x01  File encryption  File encryption information.
// 0x02  File hash        File data hash.
// 0x03  File time        High precision file time.
// 0x04  File version     File version number.
// 0x05  Redirection      File system redirection.
// 0x06  Unix owner       Unix owner and group information.
// 0x07  Service data     Service header data array.