pub mod dictionary;
pub mod kmer_model;

pub use dictionary::ReferenceDictionary;
pub use kmer_model::KmerModel;

use crate::error::Result;
use crate::types::ReferenceSequence;
use std::path::Path;

pub fn load_fasta<P: AsRef<Path>>(path: P) -> Result<Vec<ReferenceSequence>> {
    dictionary::load_fasta(path)
}

pub fn generate_kmers(sequence: &[u8], k: usize) -> Vec<String> {
    kmer_model::generate_kmers(sequence, k)
}
