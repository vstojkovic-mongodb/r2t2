use bson::document::ValueAccessError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("EOF")]
    EOF,

    #[error("error reading the FTDC file")]
    IO(#[from] std::io::Error),

    #[error("error parsing BSON")]
    BSON(#[from] bson::de::Error),

    #[error("unrecognized chunk type: {0}")]
    UnknownChunkType(i32),

    #[error("error extracting FTDC data from BSON")]
    InvalidDocumentFormat(#[from] ValueAccessError),

    #[error("error decoding FTDC data")]
    InvalidNumericFormat(leb128::read::Error),
}

impl From<leb128::read::Error> for Error {
    fn from(err: leb128::read::Error) -> Self {
        match err {
            leb128::read::Error::IoError(err) => Self::IO(err),
            err @ leb128::read::Error::Overflow => Self::InvalidNumericFormat(err),
        }
    }
}
