
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
pub struct Fast5Reader {
    file: File,
    read_groups: Vec<String>,
    current_idx: usize,
}

#[cfg(feature = "hdf5")]
impl Fast5Reader {
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self> {
        let file = File::open(path.as_ref())?;
        let read_groups = Self::find_read_groups(&file)?;
        Ok(Self {
            file,
            read_groups,
            current_idx: 0,
        })
    }

    fn find_read_groups(file: &File) -> Result<Vec<String>> {
        let mut groups = Vec::new();
        
        if let Ok(root) = file.group("/") {
            for member in root.members()? {
                if let Ok(group) = root.group(&member) {
                    if group.name().contains("read_") || group.name().contains("Read_") {
                        groups.push(group.name().to_string());
                    }
                }
            }
        }

        if groups.is_empty() {
            if let Ok(raw_group) = file.group("/Raw") {
                for member in raw_group.members()? {
                    groups.push(format!("/Raw/{}", member));
                }
            }
        }

        Ok(groups)
    }

    fn extract_signal(&self, group_path: &str) -> Result<RawSignal> {
        let group = self
            .file
            .group(group_path)
            .map_err(|e| NanoDtwError::InvalidFast5(format!(
                "Failed to open group {}: {}", group_path, e
            )))?;

        let read_id = Self::extract_read_id(&group)?;
        let samples = Self::extract_samples(&group)?;
        let sample_rate = Self::extract_sample_rate(&group)?;

        let duration = if sample_rate > 0 {
            Duration::from_secs_f64(samples.len() as f64 / sample_rate as f64)
        } else {
            Duration::from_secs(0)
        };

        let (channel, well) = Self::extract_channel_well(&group)?;

        Ok(RawSignal {
            read_id,
            samples,
            sample_rate,
            duration,
            channel,
            well,
        })
    }

    fn extract_read_id(group: &Group) -> Result<String> {
        if let Ok(attr) = group.attr("read_id") {
            if let Ok(id) = attr.read_scalar::<String>() {
                return Ok(id);
            }
        }
        
        if let Ok(ds) = group.dataset("read_id") {
            if let Ok(id) = ds.read_scalar::<String>() {
                return Ok(id);
            }
        }

        let name = group.name();
        if let Some(idx) = name.rfind("read_") {
            return Ok(name[idx + 5..].to_string());
        }

        Err(NanoDtwError::InvalidFast5("Could not extract read_id".to_string()))
    }

    fn extract_samples(group: &Group) -> Result<Vec<f32>> {
        if let Ok(signal_group) = group.group("Signal") {
            if let Ok(ds) = signal_group.dataset("Signal") {
                return Self::read_signal_dataset(&ds);
            }
        }

        if let Ok(ds) = group.dataset("Raw/Signal") {
            return Self::read_signal_dataset(&ds);
        }

        if let Ok(ds) = group.dataset("Signal") {
            return Self::read_signal_dataset(&ds);
        }

        Err(NanoDtwError::InvalidFast5(
            "Could not find signal dataset".to_string()
        ))
    }

    fn read_signal_dataset(ds: &Dataset) -> Result<Vec<f32>> {
        let dtype = ds.dtype()?;
        if dtype == hdf5::types::Type::I16 {
            let data: Vec<i16> = ds.read()?;
            Ok(data.iter().map(|&x| x as f32).collect())
        } else if dtype == hdf5::types::Type::I32 {
            let data: Vec<i32> = ds.read()?;
            Ok(data.iter().map(|&x| x as f32).collect())
        } else if dtype == hdf5::types::Type::F32 {
            Ok(ds.read::<f32, _>()?)
        } else if dtype == hdf5::types::Type::F64 {
            let data: Vec<f64> = ds.read()?;
            Ok(data.iter().map(|&x| x as f32).collect())
        } else {
            Err(NanoDtwError::InvalidFast5(format!(
                "Unsupported signal data type: {:?}", dtype
            )))
        }
    }

    fn extract_sample_rate(group: &Group) -> Result<u32> {
        if let Ok(attr) = group.attr("sampling_rate") {
            if let Ok(rate) = attr.read_scalar::<u32>() {
                return Ok(rate);
            }
            if let Ok(rate) = attr.read_scalar::<f32>() {
                return Ok(rate as u32);
            }
        }

        if let Ok(attr) = group.attr("sample_rate") {
            if let Ok(rate) = attr.read_scalar::<u32>() {
                return Ok(rate);
            }
        }

        if let Ok(tracking) = group.group("tracking_id") {
            if let Ok(attr) = tracking.attr("sample_rate") {
                if let Ok(rate) = attr.read_scalar::<f32>() {
                    return Ok(rate as u32);
                }
            }
        }

        Ok(4000)
    }

    fn extract_channel_well(group: &Group) -> Result<(u16, u8)> {
        let mut channel = 0u16;
        let mut well = 0u8;

        if let Ok(attr) = group.attr("channel") {
            if let Ok(c) = attr.read_scalar::<u16>() {
                channel = c;
            }
        }

        if let Ok(attr) = group.attr("well") {
            if let Ok(w) = attr.read_scalar::<u8>() {
                well = w;
            }
        }

        if let Ok(tracking) = group.group("tracking_id") {
            if let Ok(attr) = tracking.attr("channel_number") {
                if let Ok(c) = attr.read_scalar::<u16>() {
                    channel = c;
                }
            }
        }

        Ok((channel, well))
    }
}

#[cfg(feature = "hdf5")]
impl super::SignalReader for Fast5Reader {
    fn read_all(&mut self) -> Result<Vec<RawSignal>> {
        let mut signals = Vec::with_capacity(self.read_groups.len());
        for group_path in &self.read_groups {
            match self.extract_signal(group_path) {
                Ok(signal) => signals.push(signal),
                Err(e) => {
                    log::warn!("Failed to read {}: {}", group_path, e);
                }
            }
        }
        Ok(signals)
    }

    fn read_batch(&mut self, batch_size: usize) -> Result<Vec<RawSignal>> {
        let end = std::cmp::min(self.current_idx + batch_size, self.read_groups.len());
        let mut signals = Vec::with_capacity(end - self.current_idx);
        
        while self.current_idx < end {
            let group_path = &self.read_groups[self.current_idx];
            match self.extract_signal(group_path) {
                Ok(signal) => signals.push(signal),
                Err(e) => {
                    log::warn!("Failed to read {}: {}", group_path, e);
                }
            }
            self.current_idx += 1;
        }
        Ok(signals)
    }

    fn len(&self) -> usize {
        self.read_groups.len()
    }

    fn is_empty(&self) -> bool {
        self.read_groups.is_empty()
    }
}
