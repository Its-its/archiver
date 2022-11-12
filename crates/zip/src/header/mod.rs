mod central_directory_file;
mod end_of_central_directory;
mod local_file;

pub use central_directory_file::*;
pub use end_of_central_directory::*;
pub use local_file::*;

// 4.4.1.1  All fields unless otherwise noted are unsigned and stored in Intel low-byte:high-byte, low-word:high-word order.
// 4.4.1.2  String fields are not null terminated, since the length is given explicitly.
// 4.4.1.3  The entries in the central directory MAY NOT necessarily be in the same order that files appear in the .ZIP file.
// 4.4.1.4  If one of the fields in the end of central directory record is too small to hold required data, the field SHOULD be set to -1 (0xFFFF or 0xFFFFFFFF) and the ZIP64 format record SHOULD be created.
// 4.4.1.5  The end of central directory record and the Zip64 end of central directory locator record MUST reside on the same disk when splitting or spanning an archive.
