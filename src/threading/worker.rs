use crate::dtw::DtwAligner;
use crate::error::Result;
use crate::reference::ReferenceDictionary;
use crate::types::{DtwResult, RawSignal};
use crossbeam_channel::{Receiver, Sender};
use std::sync::Arc;
use std::thread;

pub struct WorkerPool {
    threads: Vec<thread::JoinHandle<()>>,
    num_workers: usize,
}

impl WorkerPool {
    pub fn new(
        num_workers: usize,
        reference: Arc<ReferenceDictionary>,
        signal_rx: Receiver<RawSignal>,
        result_tx: Sender<DtwResult>,
        aligner: DtwAligner,
    ) -> Result<Self> {
        let mut threads = Vec::with_capacity(num_workers);

        for _ in 0..num_workers {
            let rx = signal_rx.clone();
            let tx = result_tx.clone();
            let reference = Arc::clone(&reference);
            let aligner = aligner.clone();
            let handle = thread::spawn(move || {
                Self::worker_loop(rx, tx, reference, aligner);
            });
            threads.push(handle);
        }

        Ok(Self { threads, num_workers })
    }

    fn worker_loop(
        rx: Receiver<RawSignal>,
        tx: Sender<DtwResult>,
        reference: Arc<ReferenceDictionary>,
        aligner: DtwAligner,
    ) {
        while let Ok(signal) = rx.recv() {
            Self::process_signal(&signal, &reference, &aligner, &tx);
        }
    }

    fn process_signal(
        signal: &RawSignal,
        reference: &ReferenceDictionary,
        aligner: &DtwAligner,
        tx: &Sender<DtwResult>,
    ) {
        use crate::dtw::banded::MAX_SIGNAL_LENGTH;

        if signal.samples.len() < 50 {
            log::debug!("Signal too short for {}, skipping", signal.read_id);
            return;
        }

        if signal.samples.len() > MAX_SIGNAL_LENGTH {
            log::warn!("Signal too long for {}: {} > max {}, will truncate", 
                signal.read_id, signal.samples.len(), MAX_SIGNAL_LENGTH);
        }

        let processed_samples: Vec<f32> = if signal.samples.len() > MAX_SIGNAL_LENGTH {
            signal.samples.iter().take(MAX_SIGNAL_LENGTH).copied().collect()
        } else {
            signal.samples.clone()
        };

        let mut best_result: Option<DtwResult> = None;
        let mut best_distance = f32::MAX;

        for seq in reference.iter() {
            let ref_currents: Vec<f32> = seq.kmers.iter().map(|k| k.expected_current).collect();
            
            if ref_currents.is_empty() {
                continue;
            }

            let processed_ref: Vec<f32> = if ref_currents.len() > MAX_SIGNAL_LENGTH {
                ref_currents.iter().take(MAX_SIGNAL_LENGTH).copied().collect()
            } else {
                ref_currents.clone()
            };

            match aligner.align(&processed_samples, &processed_ref) {
                Ok(result) => {
                    let normalized_dist = result.total_distance / result.path_length as f32;
                    if normalized_dist < best_distance {
                        best_distance = normalized_dist;
                        let mapped_seq = Self::extract_mapped_sequence(&result, seq);
                        let mapping_quality = crate::utils::calculate_quality_score(
                            1.0 / (normalized_dist + 1e-6)
                        );
                        
                        best_result = Some(DtwResult {
                            read_id: signal.read_id.clone(),
                            reference_id: seq.id.clone(),
                            total_distance: result.total_distance,
                            normalized_distance: normalized_dist,
                            path_length: result.path_length,
                            alignment_path: result.alignment_path,
                            mapped_sequence: mapped_seq,
                            mapping_quality,
                            signal_start: result.signal_start,
                            signal_end: result.signal_end,
                            reference_start: result.reference_start,
                            reference_end: result.reference_end,
                        });
                    }
                }
                Err(e) => {
                    log::warn!("DTW failed for {} on {}: {}", signal.read_id, seq.id, e);
                }
            }
        }

        if let Some(result) = best_result {
            if let Err(e) = tx.send(result) {
                log::error!("Failed to send result: {}", e);
            }
        }
    }

    fn extract_mapped_sequence(
        dtw_result: &crate::dtw::DtwAlignmentResult,
        reference: &crate::types::ReferenceSequence,
    ) -> String {
        let mut bases = Vec::new();
        let mut last_ref_idx = usize::MAX;

        for point in &dtw_result.alignment_path {
            if point.reference_idx != last_ref_idx && point.reference_idx < reference.kmers.len() {
                let kmer = &reference.kmers[point.reference_idx];
                if !kmer.kmer.is_empty() {
                    bases.push(kmer.kmer.as_bytes()[0] as char);
                }
                last_ref_idx = point.reference_idx;
            }
        }

        bases.into_iter().collect()
    }

    pub fn join(self) {
        for handle in self.threads {
            let _ = handle.join();
        }
    }

    pub fn num_workers(&self) -> usize {
        self.num_workers
    }
}
