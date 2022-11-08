// The .ZIP File Format Specification documents the following compression methods:
//     Store (no compression),
//     Shrink (LZW),
//     Reduce (levels 1–4; LZ77 + probabilistic),
//     Implode,
//     Deflate,
//     Deflate64,
//     bzip2,
//     LZMA,
//     WavPack,
//     PPMd,
//     LZ77 variant provided by IBM z/OS CMPSC instruction.
// The most commonly used compression method is DEFLATE, which is described in IETF RFC 1951.

// Other methods mentioned, but not documented in detail in the specification include:
//     PKWARE DCL Implode (old IBM TERSE),
//     new IBM TERSE, IBM LZ77 z Architecture (PFS),
//     and a JPEG variant. A "Tokenize" method was reserved for a third party, but support was never added.

// The word Implode is overused by PKWARE: the DCL/TERSE Implode is distinct from the old PKZIP Implode, a predecessor to Deflate.
// The DCL Implode is undocumented partially due to its proprietary nature held by IBM, but Mark Adler has nevertheless provided a decompressor called "blast" alongside zlib.


// 4.3.4 Compression MUST NOT be applied to a "local file header", an "encryption
//    header", or an "end of central directory record".  Individual "central
//    directory records" MUST NOT be compressed, but the aggregate of all central
//    directory records MAY be compressed.

// 4.4.5 compression method: (2 bytes)
//     0 - The file is stored (no compression)
//     1 - The file is Shrunk
//     2 - The file is Reduced with compression factor 1
//     3 - The file is Reduced with compression factor 2
//     4 - The file is Reduced with compression factor 3
//     5 - The file is Reduced with compression factor 4
//     6 - The file is Imploded
//     7 - Reserved for Tokenizing compression algorithm
//     8 - The file is Deflated
//     9 - Enhanced Deflating using Deflate64(tm)
//     10 - PKWARE Data Compression Library Imploding (old IBM TERSE)
//     11 - Reserved by PKWARE
//     12 - File is compressed using BZIP2 algorithm
//     13 - Reserved by PKWARE
//     14 - LZMA
//     15 - Reserved by PKWARE
//     16 - IBM z/OS CMPSC Compression
//     17 - Reserved by PKWARE
//     18 - File is compressed using IBM TERSE (new)
//     19 - IBM LZ77 z Architecture
//     20 - deprecated (use method 93 for zstd)
//     93 - Zstandard (zstd) Compression
//     94 - MP3 Compression
//     95 - XZ Compression
//     96 - JPEG variant
//     97 - WavPack compressed data
//     98 - PPMd version I, Rev 1
//     99 - AE-x encryption marker (see APPENDIX E)

use std::io::{Cursor, Read};

use flate2::read::DeflateDecoder;
use num_enum::{TryFromPrimitive, IntoPrimitive};

use crate::Result;


#[derive(Debug, Clone, Copy, PartialEq, Eq, TryFromPrimitive, IntoPrimitive)]
#[repr(u16)]
pub enum CompressionType {
    None = 0,
    Shrunk,
    ReducedCF1,
    ReducedCF2,
    ReducedCF3,
    ReducedCF4,
    Imploded,
    Tokenizing,
    Deflate,
    Deflate64,
    IbmTerseOld,
    Bzip2 = 12,
    Lzma = 14,
    IbmZosCmpsc = 16,
    IbmTerseNew = 18,
    IbmLz77z = 19,
    // TODO: Use 93
    DeprecatedZstd = 20,
    Zstd = 93,
    Mp3,
    Xz,
    Jpeg,
    WavPack,
    PPMd,
    Aex,
}

impl CompressionType {
    pub fn decompress(self, value: Vec<u8>) -> Result<String> {
        let res = match self {
            Self::None => String::from_utf8(value)?,

            Self::Deflate => {
                let mut decoder = DeflateDecoder::new(Cursor::new(value));

                let mut s = String::new();
                decoder.read_to_string(&mut s)?;

                s
            }

            v => unimplemented!("Compression Type: {v:?}")
        };

        Ok(res)
    }
}