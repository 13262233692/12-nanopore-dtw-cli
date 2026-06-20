
use crate::error::Result;
use crate::reference::kmer_model::KmerModel;
use crate::types::RawSignal;
use rand::Rng;
use rand_distr::{Normal, Distribution};
use std::path::Path;
use std::time::Duration;

pub struct MockSignalReader {
    read_count: usize,
    current_idx: usize,
    kmer_model: KmerModel,
    reference_sequence: Vec<u8>,
    signal_length_range: (usize, usize),
    sample_rate: u32,
    drift_rate: f32,
    noise_level: f32,
}

impl MockSignalReader {
    pub fn new(read_count: usize) -> Self {
        let kmer_model = KmerModel::default_r94();
        let reference_sequence = Self::generate_reference(2000);
        
        Self {
            read_count,
            current_idx: 0,
            kmer_model,
            reference_sequence,
            signal_length_range: (5000, 20000),
            sample_rate: 4000,
            drift_rate: 0.0001,
            noise_level: 3.0,
        }
    }

    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path_str = path.as_ref().display().to_string();
        
        let mut read_count = 10;
        if let Some(num_str) = path_str.split('_').last() {
            if let Ok(n) = num_str.parse::<usize>() {
                read_count = n;
            }
        }
        
        Ok(Self::new(read_count))
    }

    fn generate_reference(length: usize) -> Vec<u8> {
        let bases = [b'A', b'C', b'G', b'T'];
        let mut rng = rand::thread_rng();
        (0..length)
            .map(|_| bases[rng.gen_range(0..4)])
            .collect()
    }

    fn generate_signal(&self, read_idx: usize) -> Result<RawSignal> {
        let mut rng = rand::thread_rng();
        
        let signal_length = rng.gen_range(
            self.signal_length_range.0..=self.signal_length_range.1
        );
        
        let ref_start = rng.gen_range(
            0..self.reference_sequence.len().saturating_sub(500)
        );
        let ref_length = rng.gen_range(200..500);
        let ref_end = (ref_start + ref_length).min(self.reference_sequence.len());
        let read_ref = &self.reference_sequence[ref_start..ref_end];
        
        let mut signal = Vec::with_capacity(signal_length);
        let k = 6;
        
        let normal = Normal::new(0.0, self.noise_level as f64).unwrap();
        
        for i in 0..signal_length {
            let drift = (i as f32) * self.drift_rate;
            
            let ref_pos = (i as f32 * (read_ref.len() as f32 / signal_length as f32)) as usize;
            let kmer_start = ref_pos.saturating_sub(k / 2);
            let kmer_end = (kmer_start + k).min(read_ref.len());
            
            let current_level = if kmer_end > kmer_start && kmer_end - kmer_start == k {
                let kmer_bytes = &read_ref[kmer_start..kmer_end];
                let kmer_str = String::from_utf8_lossy(kmer_bytes);
                self.kmer_model.get_expected_current(&kmer_str)
                    .unwrap_or(100.0)
            } else {
                100.0
            };
            
            let noise = normal.sample(&mut rng) as f32;
            let sample = current_level + drift + noise;
            
            signal.push(sample);
        }
        
        let duration = if self.sample_rate > 0 {
            Duration::from_secs_f64(signal.len() as f64 / self.sample_rate as f64)
        } else {
            Duration::from_secs(0)
        };
        
        let channel = rng.gen_range(1..=512);
        let well = rng.gen_range(1..=4);
        
        Ok(RawSignal {
            read_id: format!("mock_read_{:08x}", read_idx),
            samples: signal,
            sample_rate: self.sample_rate,
            duration,
            channel,
            well,
        })
    }
}

impl super::SignalReader for MockSignalReader {
    fn read_all(&mut self) -> Result<Vec<RawSignal>> {
        let mut signals = Vec::with_capacity(self.read_count);
        for idx in 0..self.read_count {
            match self.generate_signal(idx) {
                Ok(signal) => signals.push(signal),
                Err(e) => {
                    log::warn!("Failed to generate mock read {}: {}", idx, e);
                }
            }
        }
        Ok(signals)
    }

    fn read_batch(&mut self, batch_size: usize) -> Result<Vec<RawSignal>> {
        let end = std::cmp::min(self.current_idx + batch_size, self.read_count);
        let mut signals = Vec::with_capacity(end - self.current_idx);
        
        while self.current_idx < end {
            match self.generate_signal(self.current_idx) {
                Ok(signal) => signals.push(signal),
                Err(e) => {
                    log::warn!("Failed to generate mock read {}: {}", self.current_idx, e);
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
