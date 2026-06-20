
#[cfg(feature = "hdf5")]
use crate::error::{NanoDtwError, Result};
#[cfg(feature = "hdf5")]
use crate::types::RawSignal;
#[cfg(feature = "hdf5")]
use hdf5::{File, Group, Dataset};
#[cfg(feature = "hdf5")]
use std::path::Path;
#[cfg(feature = "hdf5")]
use std::time::Duration;

#[cfg(feature = "hdf5")]
pub struct Pod5Reader {
    file: File,
    read_count: usize,
    current_idx: usize,
}

#[cfg(feature = "hdf5")]
impl Pod5Reader {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = File::open(path.as_ref())?;
        let read_count = Self::get_read_count(&file)?;
        Ok(Self {
            file,
            read_count,
            current_idx: 0,
        })
    }

    fn get_read_count(file: &File) -> Result<usize> {
        if let Ok(reads) = file.group("/reads") {
            if let Ok(ds) = reads.dataset("read_id") {
                return Ok(ds.shape()[0]);
            }
        }

        if let Ok(records) = file.group("/records") {
            if let Ok(ds) = records.dataset("read_id") {
                return Ok(ds.shape()[0]);
            }
        }

        Ok(0)
    }

    fn extract_signal(&self, idx: usize) -> Result<RawSignal> {
        let read_id = self.get_read_id(idx)?;
        let (samples, sample_rate) = self.get_signal(idx)?;
        let duration = if sample_rate > 0 {
            Duration::from_secs_f64(samples.len() as f64 / sample_rate as f64)
        } else {
            Duration::from_secs(0)
        };
        let (channel, well) = self.get_channel_well(idx)?;

        Ok(RawSignal {
            read_id,
            samples,
            sample_rate,
            duration,
            channel,
            well,
        })
    }

    fn get_read_id(&self, idx: usize) -> Result<String> {
        if let Ok(reads) = self.file.group("/reads") {
            if let Ok(ds) = reads.dataset("read_id") {
                let ids: Vec<String> = ds.read()?;
                if idx < ids.len() {
                    return Ok(ids[idx].clone());
                }
            }
        }

        if let Ok(records) = self.file.group("/records") {
            if let Ok(ds) = records.dataset("read_id") {
                let ids: Vec<String> = ds.read()?;
                if idx < ids.len() {
                    return Ok(ids[idx].clone());
                }
            }
        }

        Ok(format!("read_{}", idx))
    }

    fn get_signal(&self, idx: usize) -> Result<(Vec<f32>, u32)> {
        let mut signal: Vec<f32> = Vec::new();
        let mut sample_rate = 4000u32;

        if let Ok(reads) = self.file.group("/reads") {
            if let Ok(ds) = reads.dataset("signal") {
                let shape = ds.shape();
                if shape.len() == 1 {
                    let all_signal: Vec<i16> = ds.read()?;
                    let start = self.get_signal_start(idx, &reads)?;
                    let count = self.get_signal_count(idx, &reads)?;
                    if start + count <= all_signal.len() {
                        signal = all_signal[start..start + count]
                            .iter()
                            .map(|&x| x as f32)
                            .collect();
                    }
                }
            }

            if let Ok(attr) = reads.attr("sample_rate") {
                if let Ok(rate) = attr.read_scalar::<f32>() {
                    sample_rate = rate as u32;
                }
            }
        }

        if signal.is_empty() {
            if let Ok(records) = self.file.group("/records") {
                if let Ok(ds) = records.dataset("signal") {
                    let all_signal: Vec<i16> = ds.read()?;
                    let row_size = all_signal.len() / self.read_count.max(1);
                    let start = idx * row_size;
                    let end = (idx + 1) * row_size;
                    if end <= all_signal.len() {
                        signal = all_signal[start..end]
                            .iter()
                            .map(|&x| x as f32)
                            .collect();
                    }
                }
            }
        }

        Ok((signal, sample_rate))
    }

    fn get_signal_start(&self, idx: usize, reads: &Group) -> Result<usize> {
        if let Ok(ds) = reads.dataset("signal_chunks") {
            let chunks: Vec<u64> = ds.read()?;
            if idx < chunks.len() {
                return Ok(chunks[idx] as usize);
            }
        }

        if let Ok(ds) = reads.dataset("chunk_start") {
            let starts: Vec<u64> = ds.read()?;
            if idx < starts.len() {
                return Ok(starts[idx] as usize);
            }
        }

        Err(NanoDtwError::InvalidPod5(
            "Could not find signal start".to_string()
        ))
    }

    fn get_signal_count(&self, idx: usize, reads: &Group) -> Result<usize> {
        if let Ok(ds) = reads.dataset("chunk_lengths") {
            let lengths: Vec<u32> = ds.read()?;
            if idx < lengths.len() {
                return Ok(lengths[idx] as usize);
            }
        }

        if let Ok(ds) = reads.dataset("num_samples") {
            let counts: Vec<u32> = ds.read()?;
            if idx < counts.len() {
                return Ok(counts[idx] as usize);
            }
        }

        Err(NanoDtwError::InvalidPod5(
            "Could not find signal count".to_string()
        ))
    }

    fn get_channel_well(&self, idx: usize) -> Result<(u16, u8)> {
        let mut channel = 0u16;
        let mut well = 0u8;

        if let Ok(reads) = self.file.group("/reads") {
            if let Ok(ds) = reads.dataset("channel") {
                let channels: Vec<u16> = ds.read()?;
                if idx < channels.len() {
                    channel = channels[idx];
                }
            }

            if let Ok(ds) = reads.dataset("well") {
                let wells: Vec<u8> = ds.read()?;
                if idx < wells.len() {
                    well = wells[idx];
                }
            }
        }

        Ok((channel, well))
    }
}

#[cfg(feature = "hdf5")]
impl super::SignalReader for Pod5Reader {
    fn read_all(&mut self) -> Result<Vec<RawSignal>> {
        let mut signals = Vec::with_capacity(self.read_count);
        for idx in 0..self.read_count {
            match self.extract_signal(idx) {
                Ok(signal) => signals.push(signal),
                Err(e) => {
                    log::warn!("Failed to read read {}: {}", idx, e);
                }
            }
        }
        Ok(signals)
    }

    fn read_batch(&mut self, batch_size: usize) -> Result<Vec<RawSignal>> {
        let end = std::cmp::min(self.current_idx + batch_size, self.read_count);
        let mut signals = Vec::with_capacity(end - self.current_idx);
        
        while self.current_idx < end {
            match self.extract_signal(self.current_idx) {
                Ok(signal) => signals.push(signal),
                Err(e) => {
                    log::warn!("Failed to read read {}: {}", self.current_idx, e);
                }
            }
            self.current_idx += 1;
        }
        Ok(signals)
    }

    fn len(&self) -> usize {
        self.read_count
    }

    fn is_empty(&self) -> bool {
        self.read_count == 0
    }
}
