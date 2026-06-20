use crate::error::{NanoDtwError, Result};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

pub struct KmerModel {
    kmer_size: usize,
    currents: HashMap<String, (f32, f32)>,
    default_current: (f32, f32),
}

impl KmerModel {
    pub fn new(kmer_size: usize) -> Self {
        Self {
            kmer_size,
            currents: HashMap::new(),
            default_current: (90.0, 5.0),
        }
    }

    pub fn load_from_file<P: AsRef<Path>>(path: P, kmer_size: usize) -> Result<Self> {
        let file = File::open(path.as_ref())?;
        let reader = BufReader::new(file);
        let mut model = Self::new(kmer_size);

        for (line_num, line) in reader.lines().enumerate() {
            let line = line?;
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 3 {
                return Err(NanoDtwError::InvalidReference(format!(
                    "Invalid kmer model line {}: {}",
                    line_num + 1,
                    line
                )));
            }

            let kmer = parts[0].to_string();
            if kmer.len() != kmer_size {
                return Err(NanoDtwError::InvalidReference(format!(
                    "Kmer size mismatch on line {}: expected {}, got {}",
                    line_num + 1,
                    kmer_size,
                    kmer.len()
                )));
            }

            let mean: f32 = parts[1]
                .parse()
                .map_err(|_| NanoDtwError::InvalidReference(format!(
                    "Invalid mean on line {}",
                    line_num + 1
                )))?;
            let std_dev: f32 = parts[2]
                .parse()
                .map_err(|_| NanoDtwError::InvalidReference(format!(
                    "Invalid std_dev on line {}",
                    line_num + 1
                )))?;

            model.currents.insert(kmer, (mean, std_dev));
        }

        if model.currents.is_empty() {
            return Err(NanoDtwError::InvalidReference(
                "No kmer entries found in model file".to_string(),
            ));
        }

        Ok(model)
    }

    pub fn default_r94() -> Self {
        Self::load_r9_model(6)
    }

    pub fn load_r9_model(kmer_size: usize) -> Self {
        let mut model = Self::new(kmer_size);
        let bases = ['A', 'T', 'C', 'G'];

        if kmer_size == 5 {
            for b1 in &bases {
                for b2 in &bases {
                    for b3 in &bases {
                        for b4 in &bases {
                            for b5 in &bases {
                                let kmer = format!("{}{}{}{}{}", b1, b2, b3, b4, b5);
                                let mean = Self::calculate_r9_current(&kmer);
                                let std_dev = 1.5 + (mean * 0.02);
                                model.currents.insert(kmer, (mean, std_dev));
                            }
                        }
                    }
                }
            }
        }

        model
    }

    fn calculate_r9_current(kmer: &str) -> f32 {
        let mut base_sum = 0.0;
        for (i, base) in kmer.chars().enumerate() {
            let weight = match i {
                0 => 0.15,
                1 => 0.25,
                2 => 0.3,
                3 => 0.2,
                4 => 0.1,
                _ => 1.0 / kmer.len() as f32,
            };
            let value = match base {
                'A' | 'a' => 85.0,
                'T' | 't' => 82.0,
                'C' | 'c' => 90.0,
                'G' | 'g' => 98.0,
                _ => 88.0,
            };
            base_sum += weight * value;
        }
        base_sum
    }

    pub fn get_current(&self, kmer: &str) -> (f32, f32) {
        self.currents
            .get(kmer)
            .copied()
            .unwrap_or(self.default_current)
    }

    pub fn get_expected_current(&self, kmer: &str) -> Option<f32> {
        self.currents
            .get(kmer)
            .map(|(mean, _)| *mean)
    }

    pub fn kmer_size(&self) -> usize {
        self.kmer_size
    }

    pub fn len(&self) -> usize {
        self.currents.len()
    }

    pub fn is_empty(&self) -> bool {
        self.currents.is_empty()
    }

    pub fn contains(&self, kmer: &str) -> bool {
        self.currents.contains_key(kmer)
    }
}

pub fn generate_kmers(sequence: &[u8], k: usize) -> Vec<String> {
    if sequence.len() < k {
        return Vec::new();
    }

    let mut kmers = Vec::with_capacity(sequence.len() - k + 1);
    for i in 0..=sequence.len() - k {
        kmers.push(String::from_utf8_lossy(&sequence[i..i + k]).to_string());
    }
    kmers
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_kmers() {
        let seq = b"ATCGATCG";
        let kmers = generate_kmers(seq, 3);
        assert_eq!(kmers.len(), 6);
        assert_eq!(kmers[0], "ATC");
        assert_eq!(kmers[1], "TCG");
        assert_eq!(kmers[5], "TCG");
    }

    #[test]
    fn test_kmer_model_r9() {
        let model = KmerModel::load_r9_model(5);
        assert_eq!(model.len(), 1024);
        let (mean, std) = model.get_current("AAAAA");
        assert!(mean > 80.0 && mean < 100.0);
        assert!(std > 0.0);
    }
}
