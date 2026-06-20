use serde::{Deserialize, Serialize};
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawSignal {
    pub read_id: String,
    pub samples: Vec<f32>,
    pub sample_rate: u32,
    pub duration: Duration,
    pub channel: u16,
    pub well: u8,
}

impl RawSignal {
    #[inline]
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.samples.is_empty()
    }

    #[inline]
    pub fn mean(&self) -> f32 {
        if self.samples.is_empty() {
            return 0.0;
        }
        let sum: f32 = self.samples.iter().sum();
        sum / self.samples.len() as f32
    }

    #[inline]
    pub fn std_dev(&self) -> f32 {
        if self.samples.is_empty() {
            return 0.0;
        }
        let mean = self.mean();
        let variance: f32 = self
            .samples
            .iter()
            .map(|&x| (x - mean).powi(2))
            .sum::<f32>()
            / self.samples.len() as f32;
        variance.sqrt()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceKmer {
    pub kmer: String,
    pub expected_current: f32,
    pub std_dev: f32,
    pub level_mean: f32,
    pub dwell_time: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReferenceSequence {
    pub id: String,
    pub name: String,
    pub length: usize,
    pub sequence: Vec<u8>,
    pub kmers: Vec<ReferenceKmer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DtwPathPoint {
    pub signal_idx: usize,
    pub reference_idx: usize,
    pub distance: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DtwResult {
    pub read_id: String,
    pub reference_id: String,
    pub total_distance: f32,
    pub normalized_distance: f32,
    pub path_length: usize,
    pub alignment_path: Vec<DtwPathPoint>,
    pub mapped_sequence: String,
    pub mapping_quality: u8,
    pub signal_start: usize,
    pub signal_end: usize,
    pub reference_start: usize,
    pub reference_end: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessStats {
    pub total_reads: usize,
    pub processed_reads: usize,
    pub failed_reads: usize,
    pub total_bases: usize,
    pub elapsed: Duration,
    pub reads_per_second: f64,
    pub bases_per_second: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileFormat {
    Fast5,
    Pod5,
    Sam,
    Bam,
    Mock,
    Unknown,
}

impl FileFormat {
    pub fn from_path<P: AsRef<std::path::Path>>(path: P) -> Self {
        let ext = path
            .as_ref()
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        match ext.as_str() {
            "fast5" => FileFormat::Fast5,
            "pod5" => FileFormat::Pod5,
            "sam" => FileFormat::Sam,
            "bam" => FileFormat::Bam,
            "mock" => FileFormat::Mock,
            _ => FileFormat::Unknown,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlignmentInfo {
    pub read_id: String,
    pub flag: u16,
    pub reference: String,
    pub position: i64,
    pub mapping_quality: u8,
    pub cigar: String,
    pub sequence: String,
    pub quality: String,
    pub edit_distance: u32,
    pub alignment_score: i32,
}

impl Default for AlignmentInfo {
    fn default() -> Self {
        AlignmentInfo {
            read_id: String::new(),
            flag: 0,
            reference: String::from("*"),
            position: -1,
            mapping_quality: 0,
            cigar: String::from("*"),
            sequence: String::from("*"),
            quality: String::from("*"),
            edit_distance: 0,
            alignment_score: 0,
        }
    }
}
