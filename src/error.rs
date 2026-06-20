use thiserror::Error;

#[derive(Error, Debug)]
pub enum NanoDtwError {
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("HDF5 error: {0}")]
    Hdf5Error(String),

    #[error("Invalid file format: {0}")]
    InvalidFormat(String),

    #[error("Invalid FAST5 structure: {0}")]
    InvalidFast5(String),

    #[error("Invalid POD5 structure: {0}")]
    InvalidPod5(String),

    #[error("DTW error: {0}")]
    DtwError(String),

    #[error("Signal too short: length={0}, minimum required={1}")]
    SignalTooShort(usize, usize),

    #[error("Reference not found: {0}")]
    ReferenceNotFound(String),

    #[error("Invalid reference: {0}")]
    InvalidReference(String),

    #[error("Thread pool error: {0}")]
    ThreadPoolError(String),

    #[error("Channel error: {0}")]
    ChannelError(String),

    #[error("BAM/SAM error: {0}")]
    BamSamError(String),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Configuration error: {0}")]
    ConfigError(String),

    #[error("Operation timed out")]
    Timeout,

    #[error("Interrupted")]
    Interrupted,

    #[error("No data found")]
    NoData,

    #[error("Unsupported format: {0}")]
    UnsupportedFormat(String),

    #[error("Internal error: {0}")]
    InternalError(String),
}

pub type Result<T> = std::result::Result<T, NanoDtwError>;

#[cfg(feature = "hdf5")]
impl From<hdf5::Error> for NanoDtwError {
    fn from(e: hdf5::Error) -> Self {
        NanoDtwError::Hdf5Error(e.to_string())
    }
}

impl<T> From<crossbeam_channel::SendError<T>> for NanoDtwError {
    fn from(e: crossbeam_channel::SendError<T>) -> Self {
        NanoDtwError::ChannelError(e.to_string())
    }
}

impl From<crossbeam_channel::RecvError> for NanoDtwError {
    fn from(e: crossbeam_channel::RecvError) -> Self {
        NanoDtwError::ChannelError(e.to_string())
    }
}

impl From<clap::Error> for NanoDtwError {
    fn from(e: clap::Error) -> Self {
        NanoDtwError::ParseError(e.to_string())
    }
}
