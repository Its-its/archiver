use thiserror::Error as ThisError;
use num_enum::TryFromPrimitiveError;

pub type Result<R, E = Error> = std::result::Result<R, E>;

#[derive(Debug, ThisError)]
pub enum Error {
    #[error("IO Error: {0:?}")]
    Io(#[from] std::io::Error),

    #[error("UTF-8 Error: {0:?}")]
    Utf8(#[from] std::string::FromUtf8Error),

    #[error("Num Enum Error: {0:?}")]
    NumEnum(#[from] TryFromPrimitiveError<crate::compression::CompressionType>),

    #[error("LZMA Error: {0:?}")]
    Lzma(#[from] lzma_rs::error::Error),

    #[error("Missing End Header")]
    MissingEndHeader,
}