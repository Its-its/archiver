use std::{io::SeekFrom, fmt};

use tokio::io::{AsyncSeekExt, AsyncReadExt};

use crate::{BUFFER_SIZE, SIGNATURE_SIZE, ArchiveReader, Result, compression::CompressionType, Archive};

use super::LocalFileHeader;



pub(crate) const CENTRAL_DIR_SIG: [u8; 4] = [0x50, 0x4B, 0x01, 0x02];
pub(crate) const CENTRAL_DIR_SIZE_KNOWN: usize = 46;


/// Documents Each File
#[derive(Debug, Clone)]
pub struct CentralDirHeader {
    /// Version made by
    pub by_version: Version,
    /// Version needed to extract (minimum)
    pub min_version: VersionNeeded,
    /// General purpose bit flag
    pub gp_flag: u16,
    /// Compression method
    pub compression: CompressionType,
    /// File last modification time
    pub file_last_mod_time: u16,
    /// File last modification date
    pub file_last_mod_date: u16,
    /// CRC-32 of uncompressed data
    pub crc_32: u32,
    /// Compressed size (or 0xffffffff for ZIP64)
    pub compressed_size: u32,
    /// Uncompressed size (or 0xffffffff for ZIP64)
    pub uncompressed_size: u32,
    /// File name length (n)
    pub file_name_length: u16,
    /// Extra field length (m)
    pub extra_field_length: u16,
    /// File comment length (k)
    pub file_comment_length: u16,
    /// Disk number where file starts (or 0xffff for ZIP64)
    pub current_disk_number: u16,
    /// Internal file attributes
    pub internal_file_attr: u16,
    /// External file attributes
    pub external_file_attr: u32,
    /// Relative offset of local file header (or 0xffffffff for ZIP64). This is the number of bytes between the start of the first disk on which the file occurs, and the start of the local file header. This allows software reading the central directory to locate the position of the file inside the ZIP file.
    pub relative_offset: u32,
    /// File name
    pub file_name: String,
    /// Used to store additional information.
    ///
    /// The field consists of a sequence of header and data pairs, where the header has a 2 byte identifier and a 2 byte data size field.
    pub extra_field: Vec<(u16, u16)>,
    /// File comment
    pub file_comment: String,
}

impl CentralDirHeader {
    pub async fn parse(reader: &mut ArchiveReader<'_>, buffer: &mut [u8; BUFFER_SIZE]) -> Result<Self> {
        assert_eq!(&buffer[reader.index..reader.index + 4], &CENTRAL_DIR_SIG);

        reader.skip::<4>();

        let mut header = Self {
            by_version: chunk_to_version(reader.get_next_chunk::<2>(buffer).await?),
            min_version: VersionNeeded(reader.next_u16(buffer).await?),
            gp_flag: reader.next_u16(buffer).await?,
            compression: CompressionType::try_from(reader.next_u16(buffer).await?)?,
            file_last_mod_time: reader.next_u16(buffer).await?,
            file_last_mod_date: reader.next_u16(buffer).await?,
            crc_32: reader.next_u32(buffer).await?,
            compressed_size: reader.next_u32(buffer).await?,
            uncompressed_size: reader.next_u32(buffer).await?,
            file_name_length: reader.next_u16(buffer).await?,
            extra_field_length: reader.next_u16(buffer).await?,
            file_comment_length: reader.next_u16(buffer).await?,
            current_disk_number: reader.next_u16(buffer).await?,
            internal_file_attr: reader.next_u16(buffer).await?,
            external_file_attr: reader.next_u32(buffer).await?,
            relative_offset: reader.next_u32(buffer).await?,
            file_name: String::new(),
            extra_field: Vec::new(),
            file_comment: String::new(),
        };

        header.file_name = String::from_utf8(reader.get_chunk_amount(buffer, header.file_name_length as usize).await?)?;
        header.extra_field = reader.get_chunk_amount(buffer, header.extra_field_length as usize).await?
            .into_iter()
            .array_chunks::<4>()
            .map(|v| (
                (u16::from(v[0]) << 8) | u16::from(v[1]),
                (u16::from(v[2]) << 8) | u16::from(v[3])
            ))
            .collect();
        header.file_comment = String::from_utf8(reader.get_chunk_amount(buffer, header.file_comment_length as usize).await?)?;

        Ok(header)
    }

    pub async fn read(&self, archive: &mut Archive) -> Result<String> {
        let mut reader = ArchiveReader::init(&mut archive.file).await?;

        Ok(LocalFileHeader::parse(&mut reader, self.relative_offset as u64).await?.1)
    }
}


// Used so we don't have to have load all the files on initial open.
#[derive(Default)]
pub struct FileReaderCache {
    last_seek_pos: u64,
    // Contains a capacity for how many files we should have.
    pub(crate) files: Vec<CentralDirHeader>,
}

impl FileReaderCache {
    pub fn is_fully_cached(&self) -> bool {
        self.files.len() == self.files.capacity()
    }

    pub async fn list_files(&mut self, reader: &mut ArchiveReader<'_>) -> Result<Vec<CentralDirHeader>> {
        let mut items = self.files.clone();

        if !self.is_fully_cached() {
            while let Some(found) = self.find_next(reader).await? {
                items.push(found.clone());
            }
        }

        Ok(items)
    }

    pub async fn find_next(&mut self, reader: &mut ArchiveReader<'_>) -> Result<Option<&CentralDirHeader>> {
        if self.is_fully_cached() {
            return Ok(None);
        }

        let mut buffer = [0u8; BUFFER_SIZE];

        // TODO: Handle better. I don't want to seek if we don't need to.
        reader.seek_to(self.last_seek_pos).await?;

        loop {
            // Read updates seek position
            reader.last_read_amount = reader.file.read(&mut buffer).await?;
            reader.index = 0;

            if let Some(at_index) = reader.find_next_signature(&buffer, CENTRAL_DIR_SIG) {
                // Set our current index to where the signature starts.
                reader.index = at_index;

                // trace!("Found Header @ {} {} {:x?}", self.file.stream_position().unwrap() as usize + self.index, self.index, &buffer[self.index..self.index + 4]);

                assert_eq!(&buffer[reader.index..reader.index + 4], &CENTRAL_DIR_SIG);

                // TODO: Remove.
                if reader.index + CENTRAL_DIR_SIZE_KNOWN >= buffer.len() {
                    reader.seek_to_index(&mut buffer).await?;
                }

                let header = CentralDirHeader::parse(reader, &mut buffer).await?;

                // trace!("{header:#?}");

                self.files.push(header);

                // Seek position we're at?
                self.last_seek_pos = reader.get_seek_position().await?;

                return Ok(self.files.last());
            }

            // Nothing left to read?
            if reader.last_read_amount < buffer.len() {
                break;
            }

            // We negate the signature size to ensure we didn't get a partial previously. We remove 1 from size to prevent (end of buffer) duplicates.
            reader.file.seek(SeekFrom::Current(1 - SIGNATURE_SIZE as i64)).await?;
        }

        Ok(None)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Version {
    pub compatibility: u8,
    /// The lower byte indicates the ZIP specification version (the version of this document) supported by the software used to encode the file.
    /// The value/10 indicates the major version number,
    pub major: u8,
    /// and the value mod 10 is the minor version number.
    pub minor: u8,
}

impl Version {
    pub fn from_bytes(upper: u8, lower: u8) -> Self {
        // TODO: Validate.
        Self {
            compatibility: upper,
            major: lower / 10,
            minor: lower % 10,
        }
    }
}

fn chunk_to_version(buffer: &[u8]) -> Version {
    Version::from_bytes(buffer[1], buffer[0])
}


#[derive(Debug, Clone, Copy)]
pub struct VersionNeeded(u16);

impl VersionNeeded {
    pub fn is_file(&self) -> bool {
        !self.is_folder()
    }

    pub fn is_folder(&self) -> bool {
        // TODO: Probably more tests
        self.0 == 20
    }
}

impl fmt::Display for VersionNeeded {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

// TODO: Early 7.x (pre-7.2) versions of PKZIP incorrectly set the version needed to extract for BZIP2 compression to be 50 when it SHOULD have been 46.
// TODO: When using ZIP64 extensions, the corresponding value in the zip64 end of central directory record MUST also be set. This field SHOULD be set appropriately to indicate whether Version 1 or Version 2 format is in use.

// 4.4.3 version needed to extract (2 bytes)
//     4.4.3.1 The minimum supported ZIP specification version needed
//     to extract the file, mapped as above.  This value is based on
//     the specific format features a ZIP program MUST support to
//     be able to extract the file.  If multiple features are
//     applied to a file, the minimum version MUST be set to the
//     feature having the highest value. New features or feature
//     changes affecting the published format specification will be
//     implemented using higher version numbers than the last
//     published value to avoid conflict.

// 4.4.3.2 Current minimum feature versions are as defined below:
//     1.0 - Default value
//     1.1 - File is a volume label
//     2.0 - File is a folder (directory)
//     2.0 - File is compressed using Deflate compression
//     2.0 - File is encrypted using traditional PKWARE encryption
//     2.1 - File is compressed using Deflate64(tm)
//     2.5 - File is compressed using PKWARE DCL Implode
//     2.7 - File is a patch data set
//     4.5 - File uses ZIP64 format extensions
//     4.6 - File is compressed using BZIP2 compression*
//     5.0 - File is encrypted using DES
//     5.0 - File is encrypted using 3DES
//     5.0 - File is encrypted using original RC2 encryption
//     5.0 - File is encrypted using RC4 encryption
//     5.1 - File is encrypted using AES encryption
//     5.1 - File is encrypted using corrected RC2 encryption**
//     5.2 - File is encrypted using corrected RC2-64 encryption**
//     6.1 - File is encrypted using non-OAEP key wrapping***
//     6.2 - Central directory encryption
//     6.3 - File is compressed using LZMA
//     6.3 - File is compressed using PPMd+
//     6.3 - File is encrypted using Blowfish
//     6.3 - File is encrypted using Twofish

// 4.4.3.3 Notes on version needed to extract
//     * Early 7.x (pre-7.2) versions of PKZIP incorrectly set the
//     version needed to extract for BZIP2 compression to be 50
//     when it SHOULD have been 46.
//
//     ** Refer to the section on Strong Encryption Specification
//     for additional information regarding RC2 corrections.
//
//     *** Certificate encryption using non-OAEP key wrapping is the
//     intended mode of operation for all versions beginning with 6.1.
//     Support for OAEP key wrapping MUST only be used for
//     backward compatibility when sending ZIP files to be opened by
//     versions of PKZIP older than 6.1 (5.0 or 6.0).
//
//     + Files compressed using PPMd MUST set the version
//     needed to extract field to 6.3, however, not all ZIP
//     programs enforce this and MAY be unable to decompress
//     data files compressed using PPMd if this value is set.
//
//     When using ZIP64 extensions, the corresponding value in the
//     zip64 end of central directory record MUST also be set.
//     This field SHOULD be set appropriately to indicate whether
//     Version 1 or Version 2 format is in use.

// 4.4.4 general purpose bit flag: (2 bytes)
//     Bit 0: If set, indicates that the file is encrypted.
//     (For Method 6 - Imploding)
//     Bit 1: If the compression method used was type 6,
//             Imploding, then this bit, if set, indicates
//             an 8K sliding dictionary was used.  If clear,
//             then a 4K sliding dictionary was used.
//     Bit 2: If the compression method used was type 6,
//             Imploding, then this bit, if set, indicates
//             3 Shannon-Fano trees were used to encode the
//             sliding dictionary output.  If clear, then 2
//             Shannon-Fano trees were used.
//     (For Methods 8 and 9 - Deflating)
//     Bit 2  Bit 1
//         0      0    Normal (-en) compression option was used.
//         0      1    Maximum (-exx/-ex) compression option was used.
//         1      0    Fast (-ef) compression option was used.
//         1      1    Super Fast (-es) compression option was used.
//     (For Method 14 - LZMA)
//     Bit 1: If the compression method used was type 14,
//             LZMA, then this bit, if set, indicates
//             an end-of-stream (EOS) marker is used to
//             mark the end of the compressed data stream.
//             If clear, then an EOS marker is not present
//             and the compressed data size must be known
//             to extract.
//     Note:  Bits 1 and 2 are undefined if the compression
//             method is any other.
//     Bit 3: If this bit is set, the fields crc-32, compressed
//             size and uncompressed size are set to zero in the
//             local header.  The correct values are put in the
//             data descriptor immediately following the compressed
//             data.  (Note: PKZIP version 2.04g for DOS only
//             recognizes this bit for method 8 compression, newer
//             versions of PKZIP recognize this bit for any
//             compression method.)
//     Bit 4: Reserved for use with method 8, for enhanced
//             deflating.
//     Bit 5: If this bit is set, this indicates that the file is
//             compressed patched data.  (Note: Requires PKZIP
//             version 2.70 or greater)
//     Bit 6: Strong encryption.  If this bit is set, you MUST
//             set the version needed to extract value to at least
//             50 and you MUST also set bit 0.  If AES encryption
//             is used, the version needed to extract value MUST
//             be at least 51. See the section describing the Strong
//             Encryption Specification for details.  Refer to the
//             section in this document entitled "Incorporating PKWARE
//             Proprietary Technology into Your Product" for more
//             information.
//     Bit 7: Currently unused.
//     Bit 8: Currently unused.
//     Bit 9: Currently unused.
//     Bit 10: Currently unused.
//     Bit 11: Language encoding flag (EFS).  If this bit is set,
//             the filename and comment fields for this file
//             MUST be encoded using UTF-8. (see APPENDIX D)
//     Bit 12: Reserved by PKWARE for enhanced compression.
//     Bit 13: Set when encrypting the Central Directory to indicate
//             selected data values in the Local Header are masked to
//             hide their actual values.  See the section describing
//             the Strong Encryption Specification for details.  Refer
//             to the section in this document entitled "Incorporating
//             PKWARE Proprietary Technology into Your Product" for
//             more information.
//     Bit 14: Reserved by PKWARE for alternate streams.
//     Bit 15: Reserved by PKWARE.



// Tools that correctly read ZIP archives must scan for the end of central directory record signature, and then, as appropriate, the other, indicated, central directory records.

// Most of the signatures end with the short integer 0x 4b50, which is stored in little-endian ordering. Viewed as an ASCII string this reads "PK", the initials of the inventor Phil Katz.
// Thus, when a ZIP file is viewed in a text editor the first two bytes of the file are usually "PK".
// (DOS, OS/2 and Windows self-extracting ZIPs have an EXE before the ZIP so start with "MZ"; self-extracting ZIPs for other operating systems may similarly be preceded by executable code for extracting the archive's content on that platform.)

// Each entry stored in a ZIP archive is introduced by a local file header with information about the file such as the comment, file size and file name, followed by optional "extra" data fields, and then the possibly compressed, possibly encrypted file data.
// The "Extra" data fields are the key to the extensibility of the ZIP format.
// "Extra" fields are exploited to support the ZIP64 format, WinZip-compatible AES encryption, file attributes, and higher-resolution NTFS or Unix file timestamps.
// Other extensions are possible via the "Extra" field.
// ZIP tools are required by the specification to ignore Extra fields they do not recognize.

// The ZIP format uses specific 4-byte "signatures" to denote the various structures in the file.
// Each file entry is marked by a specific signature.
// The end of central directory record is indicated with its specific signature, and each entry in the central directory starts with the 4-byte central file header signature.

// There is no BOF or EOF marker in the ZIP specification.
// Conventionally the first thing in a ZIP file is a ZIP entry, which can be identified easily by its local file header signature.
// However, this is not necessarily the case, as this not required by the ZIP specification - most notably, a self-extracting archive will begin with an executable file header.
