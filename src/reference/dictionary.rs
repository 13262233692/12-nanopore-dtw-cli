use crate::error::{NanoDtwError, Result};
use crate::reference::kmer_model::KmerModel;
use crate::types::{ReferenceKmer, ReferenceSequence};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

pub struct ReferenceDictionary {
    sequences: Vec<ReferenceSequence>,
    id_index: HashMap<String, usize>,
    kmer_model: Option<KmerModel>,
    kmer_size: usize,
}

impl ReferenceDictionary {
    pub fn new() -> Self {
        Self {
            sequences: Vec::new(),
            id_index: HashMap::new(),
            kmer_model: None,
            kmer_size: 5,
        }
    }

    pub fn with_kmer_size(mut self, k: usize) -> Self {
        self.kmer_size = k;
        self
    }

    pub fn with_kmer_model(mut self, model: KmerModel) -> Self {
        self.kmer_model = Some(model);
        self
    }

    pub fn load_from_fasta<P: AsRef<Path>>(&mut self, path: P) -> Result<()> {
        let sequences = load_fasta(path)?;
        for seq in sequences {
            self.add_sequence(seq)?;
        }
        Ok(())
    }

    pub fn add_sequence(&mut self, mut seq: ReferenceSequence) -> Result<()> {
        if seq.kmers.is_empty() {
            seq.kmers = self.generate_kmers_for_sequence(&seq.sequence)?;
        }

        let idx = self.sequences.len();
        self.id_index.insert(seq.id.clone(), idx);
        self.sequences.push(seq);
        Ok(())
    }

    fn generate_kmers_for_sequence(&self, sequence: &[u8]) -> Result<Vec<ReferenceKmer>> {
        if sequence.len() < self.kmer_size {
            return Err(NanoDtwError::InvalidReference(format!(
                "Sequence too short: {} < {}",
                sequence.len(),
                self.kmer_size
            )));
        }

        let mut kmers = Vec::with_capacity(sequence.len() - self.kmer_size + 1);

        for i in 0..=sequence.len() - self.kmer_size {
            let kmer_bytes = &sequence[i..i + self.kmer_size];
            let kmer = String::from_utf8_lossy(kmer_bytes).to_string();

            let (expected_current, std_dev) = self.get_kmer_current(&kmer);

            kmers.push(ReferenceKmer {
                kmer: kmer.clone(),
                expected_current,
                std_dev,
                level_mean: expected_current,
                dwell_time: 1.0,
            });
        }

        Ok(kmers)
    }

    fn get_kmer_current(&self, kmer: &str) -> (f32, f32) {
        if let Some(model) = &self.kmer_model {
            model.get_current(kmer)
        } else {
            (Self::default_kmer_current(kmer), 1.0)
        }
    }

    fn default_kmer_current(kmer: &str) -> f32 {
        let mut sum = 0.0;
        for base in kmer.as_bytes() {
            sum += match base {
                b'A' | b'a' => 90.0,
                b'T' | b't' => 85.0,
                b'C' | b'c' => 95.0,
                b'G' | b'g' => 100.0,
                _ => 90.0,
            };
        }
        sum / kmer.len() as f32
    }

    pub fn get(&self, id: &str) -> Option<&ReferenceSequence> {
        self.id_index.get(id).map(|&idx| &self.sequences[idx])
    }

    pub fn iter(&self) -> std::slice::Iter<'_, ReferenceSequence> {
        self.sequences.iter()
    }

    pub fn len(&self) -> usize {
        self.sequences.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sequences.is_empty()
    }

    pub fn kmer_size(&self) -> usize {
        self.kmer_size
    }

    pub fn sequences(&self) -> &[ReferenceSequence] {
        &self.sequences
    }
}

impl Default for ReferenceDictionary {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> IntoIterator for &'a ReferenceDictionary {
    type Item = &'a ReferenceSequence;
    type IntoIter = std::slice::Iter<'a, ReferenceSequence>;

    fn into_iter(self) -> Self::IntoIter {
        self.sequences.iter()
    }
}

pub fn load_fasta<P: AsRef<Path>>(path: P) -> Result<Vec<ReferenceSequence>> {
    let file = File::open(path.as_ref())?;
    let reader = BufReader::new(file);

    let mut sequences = Vec::new();
    let mut current_id = String::new();
    let mut current_name = String::new();
    let mut current_seq = Vec::new();

    for line in reader.lines() {
        let line = line?;
        if line.is_empty() {
            continue;
        }

        if line.starts_with('>') {
            if !current_id.is_empty() {
                sequences.push(ReferenceSequence {
                    id: current_id.clone(),
                    name: current_name.clone(),
                    length: current_seq.len(),
                    sequence: std::mem::take(&mut current_seq),
                    kmers: Vec::new(),
                });
            }

            let header = &line[1..];
            let parts: Vec<&str> = header.splitn(2, |c| c == ' ' || c == '\t').collect();
            current_id = parts[0].to_string();
            current_name = if parts.len() > 1 {
                parts[1].to_string()
            } else {
                current_id.clone()
            };
            current_seq = Vec::new();
        } else {
            current_seq.extend_from_slice(line.as_bytes());
        }
    }

    if !current_id.is_empty() {
        sequences.push(ReferenceSequence {
            id: current_id,
            name: current_name,
            length: current_seq.len(),
            sequence: current_seq,
            kmers: Vec::new(),
        });
    }

    if sequences.is_empty() {
        return Err(NanoDtwError::InvalidReference(
            "No sequences found in FASTA file".to_string(),
        ));
    }

    Ok(sequences)
}
