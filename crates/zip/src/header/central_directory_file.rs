use std::io::SeekFrom;

use tokio::io::{AsyncSeekExt, AsyncReadExt};

use crate::{Archive, BUFFER_SIZE, SIGNATURE_SIZE};



pub(crate) const CENTRAL_DIR_SIG: [u8; 4] = [0x50, 0x4B, 0x01, 0x02];
pub(crate) const CENTRAL_DIR_SIZE_KNOWN: usize = 46;


/// Documents Each File
#[derive(Debug)]
pub struct CentralDirHeader {
    /// Version made by
    by_version: Version,
    /// Version needed to extract (minimum)
    // TODO: Early 7.x (pre-7.2) versions of PKZIP incorrectly set the version needed to extract for BZIP2 compression to be 50 when it SHOULD have been 46.
    // TODO: When using ZIP64 extensions, the corresponding value in the zip64 end of central directory record MUST also be set. This field SHOULD be set appropriately to indicate whether Version 1 or Version 2 format is in use.
    min_version: u16,
    /// General purpose bit flag
    gp_flag: u16,
    /// Compression method
    compression: u16,
    /// File last modification time
    file_last_mod_time: u16,
    /// File last modification date
    file_last_mod_date: u16,
    /// CRC-32 of uncompressed data
    crc_32: u32,
    /// Compressed size (or 0xffffffff for ZIP64)
    compressed_size: u32,
    /// Uncompressed size (or 0xffffffff for ZIP64)
    uncompressed_size: u32,
    /// File name length (n)
    file_name_length: u16,
    /// Extra field length (m)
    extra_field_length: u16,
    /// File comment length (k)
    file_comment_length: u16,
    /// Disk number where file starts (or 0xffff for ZIP64)
    current_disk_number: u16,
    /// Internal file attributes
    internal_file_attr: u16,
    /// External file attributes
    external_file_attr: u32,
    /// Relative offset of local file header (or 0xffffffff for ZIP64). This is the number of bytes between the start of the first disk on which the file occurs, and the start of the local file header. This allows software reading the central directory to locate the position of the file inside the ZIP file.
    pub relative_offset: u32,
    /// File name
    pub file_name: String,
    /// Extra field
    pub extra_field: Vec<u8>,
    /// File comment
    pub file_comment: String,
}

impl CentralDirHeader {
    pub async fn parse(archive: &mut Archive, buffer: &mut [u8; BUFFER_SIZE]) -> Self {
        assert_eq!(&buffer[archive.index..archive.index + 4], &CENTRAL_DIR_SIG);

        archive.skip::<4>();

        let mut header = Self {
            by_version: chunk_to_version(archive.get_next_chunk::<2>(buffer).await),
            min_version: archive.next_u16(buffer).await,
            gp_flag: archive.next_u16(buffer).await,
            compression: archive.next_u16(buffer).await,
            file_last_mod_time: archive.next_u16(buffer).await,
            file_last_mod_date: archive.next_u16(buffer).await,
            crc_32: archive.next_u32(buffer).await,
            compressed_size: archive.next_u32(buffer).await,
            uncompressed_size: archive.next_u32(buffer).await,
            file_name_length: archive.next_u16(buffer).await,
            extra_field_length: archive.next_u16(buffer).await,
            file_comment_length: archive.next_u16(buffer).await,
            current_disk_number: archive.next_u16(buffer).await,
            internal_file_attr: archive.next_u16(buffer).await,
            external_file_attr: archive.next_u32(buffer).await,
            relative_offset: archive.next_u32(buffer).await,
            file_name: String::new(),
            extra_field: Vec::new(),
            file_comment: String::new(),
        };

        header.file_name = String::from_utf8(archive.get_chunk_amount(buffer, header.file_name_length as usize).await).unwrap();
        header.extra_field = archive.get_chunk_amount(buffer, header.extra_field_length as usize).await;
        header.file_comment = String::from_utf8(archive.get_chunk_amount(buffer, header.file_comment_length as usize).await).unwrap();

        header
    }
}


// Used so we don't have to have load all the files on initial open.
#[derive(Default)]
pub struct FileReaderCache {
    last_seek_pos: u64,
    files: Vec<CentralDirHeader>,
    finished: bool
}

impl FileReaderCache {
    pub async fn find_next(&mut self, archive: &mut Archive) -> Option<&CentralDirHeader> {
        // TODO:

        let mut buffer = [0u8; BUFFER_SIZE];

        archive.file.seek(SeekFrom::Start(self.last_seek_pos)).await.unwrap();

        loop {
            // Read updates seek position
            archive.last_read_amount = archive.file.read(&mut buffer).await.unwrap();
            archive.index = 0;

            if let Some(at_index) = archive.find_next_signature(&buffer, CENTRAL_DIR_SIG) {
                // Set our current index to where the signature starts.
                archive.index = at_index;

                // println!("Found Header @ {} {} {:x?}", self.file.stream_position().unwrap() as usize + self.index, self.index, &buffer[self.index..self.index + 4]);

                assert_eq!(&buffer[archive.index..archive.index + 4], &CENTRAL_DIR_SIG);

                // TODO: Remove.
                if archive.index + CENTRAL_DIR_SIZE_KNOWN as usize >= buffer.len() {
                    archive.seek_to_index(&mut buffer).await;
                }

                let header = CentralDirHeader::parse(archive, &mut buffer).await;

                // println!("{header:#?}");

                self.files.push(header);

                self.last_seek_pos = archive.file.stream_position().await.unwrap();

                return self.files.last();
            }

            // Nothing left to read?
            if archive.last_read_amount < buffer.len() {
                break;
            }

            // We negate the signature size to ensure we didn't get a partial previously. We remove 1 from size to prevent (end of buffer) duplicates.
            archive.file.seek(SeekFrom::Current(1 - SIGNATURE_SIZE as i64)).await.unwrap();
        }

        None
    }
}



#[derive(Debug)]
struct Version {
    compatibility: u8,
    /// The lower byte indicates the ZIP specification version (the version of this document) supported by the software used to encode the file.
    /// The value/10 indicates the major version number,
    major: u8,
    /// and the value mod 10 is the minor version number.
    minor: u8,
}

impl Version {
    pub fn from_bytes(upper: u8, lower: u8) -> Self {
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
