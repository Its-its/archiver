use num_enum::TryFromPrimitiveError;
use thiserror::Error as ThisError;

pub type Result<R, E = Error> = std::result::Result<R, E>;

#[derive(Debug, ThisError)]
pub enum Error {
    #[error("IO Error: {0:?}")]
    Io(#[from] std::io::Error),

    #[error("UTF-8 Error: {0:?}")]
    Utf8(#[from] std::string::FromUtf8Error),

    // TODO: Join together.
    #[error("Num Enum Error: {0:?}")]
    NumEnumHeaderType(#[from] TryFromPrimitiveError<crate::HeaderType>),

    #[error("Num Enum Error: {0:?}")]
    NumEnumFileExtraRecord(#[from] TryFromPrimitiveError<crate::FileExtraRecordType>),

    #[error("Num Enum 4 Error: {0:?}")]
    NumEnumHeaderType4(#[from] TryFromPrimitiveError<crate::HeaderType4_0>),

    #[error("Invalid Bit Flag {name:?} => {flag:?}")]
    InvalidBitFlag { name: &'static str, flag: u64 },

    #[error("Missing Main Header")]
    MissingMainHeader,

    #[error("Missing End Header")]
    MissingEndHeader,
}
