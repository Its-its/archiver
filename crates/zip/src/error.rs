use thiserror::Error as ThisError;

pub type Result<R, E = Error> = std::result::Result<R, E>;

#[derive(Debug, ThisError)]
pub enum Error {
    #[error("IO Error: {0:?}")]
    Io(#[from] std::io::Error),

    #[error("UTF-8 Error: {0:?}")]
    Utf8(#[from] std::string::FromUtf8Error),

    #[error("Missing End Header")]
    MissingEndHeader,
}