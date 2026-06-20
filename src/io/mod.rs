
#[cfg(feature = "hdf5")]
pub mod fast5;
#[cfg(feature = "hdf5")]
pub mod pod5;
pub mod sam_bam;
pub mod mock;

#[cfg(feature = "hdf5")]
pub use fast5::Fast5Reader;
#[cfg(feature = "hdf5")]
pub use pod5::Pod5Reader;
pub use sam_bam::{AlignmentWriter, SamBamWriter};
pub use mock::MockSignalReader;

use crate::error::Result;
use crate::types::{FileFormat, RawSignal};
use std::path::Path;

pub trait SignalReader {
    fn read_all(&mut self) -> Result<Vec<RawSignal>>;
    fn read_batch(&mut self, batch_size: usize) -> Result<Vec<RawSignal>>;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool;
}

pub fn create_reader<P: AsRef<Path>>(path: P) -> Result<Box<dyn SignalReader + Send>> {
    let format = FileFormat::from_path(&path);
    match format {
        #[cfg(feature = "hdf5")]
        FileFormat::Fast5 => Ok(Box::new(Fast5Reader::open(path)?)),
        #[cfg(feature = "hdf5")]
        FileFormat::Pod5 => Ok(Box::new(Pod5Reader::open(path)?)),
        FileFormat::Mock => Ok(Box::new(MockSignalReader::from_path(path)?)),
        #[cfg(not(feature = "hdf5"))]
        _ => Ok(Box::new(MockSignalReader::from_path(path)?)),
        #[cfg(feature = "hdf5")]
        _ => Err(crate::error::NanoDtwError::UnsupportedFormat(
            path.as_ref().display().to_string(),
        )),
    }
}
