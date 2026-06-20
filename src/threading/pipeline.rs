use crate::error::Result;
use crate::io::SignalReader;
use crate::reference::ReferenceDictionary;
use crate::threading::WorkerPool;
use crate::types::{DtwResult, ProcessStats, RawSignal};
use crossbeam_channel::bounded;
use indicatif::{ProgressBar, ProgressStyle};
use std::sync::Arc;
use std::time::Instant;

pub struct ProcessingPipeline {
    num_workers: usize,
    batch_size: usize,
    channel_capacity: usize,
}

impl ProcessingPipeline {
    pub fn new(num_workers: usize, batch_size: usize, channel_capacity: usize) -> Self {
        Self {
            num_workers,
            batch_size,
            channel_capacity,
        }
    }

    pub fn run(
        &self,
        readers: Vec<Box<dyn SignalReader + Send>>,
        reference: ReferenceDictionary,
        mut output_handler: impl FnMut(DtwResult) -> Result<()>,
    ) -> Result<ProcessStats> {
        let total_reads: usize = readers.iter().map(|r| r.len()).sum();

        let (signal_tx, signal_rx) = bounded::<RawSignal>(self.channel_capacity);
        let (result_tx, result_rx) = bounded::<DtwResult>(self.channel_capacity);

        let reference_arc = Arc::new(reference);
        let aligner = crate::dtw::DtwAligner::default();

        let pool = WorkerPool::new(
            self.num_workers,
            Arc::clone(&reference_arc),
            signal_rx,
            result_tx,
            aligner,
        )?;

        let pb = ProgressBar::new(total_reads as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] \
                     {pos}/{len} reads ({eta}) | {msg}"
                )
                .unwrap()
                .progress_chars("##-"),
        );

        let start_time = Instant::now();
        let mut processed = 0usize;
        let mut failed = 0usize;
        let mut total_bases = 0usize;

        let mut readers = readers;

        for reader in readers.iter_mut() {
            loop {
                let batch = reader.read_batch(self.batch_size)?;
                if batch.is_empty() {
                    break;
                }

                for signal in batch {
                    if signal.is_empty() {
                        failed += 1;
                        pb.inc(1);
                        continue;
                    }

                    signal_tx.send(signal)?;
                }
            }
        }

        drop(signal_tx);

        while let Ok(result) = result_rx.recv() {
            total_bases += result.mapped_sequence.len();
            processed += 1;
            pb.inc(1);
            pb.set_message(format!(
                "{} bases, {} failed",
                crate::utils::format_number(total_bases),
                failed
            ));

            if let Err(e) = output_handler(result) {
                log::warn!("Output handler error: {}", e);
            }

            if processed + failed >= total_reads {
                break;
            }
        }

        pool.join();

        while let Ok(result) = result_rx.try_recv() {
            total_bases += result.mapped_sequence.len();
            processed += 1;
            pb.inc(1);
            if let Err(e) = output_handler(result) {
                log::warn!("Output handler error: {}", e);
            }
        }

        pb.finish_with_message("Processing complete");

        let elapsed = start_time.elapsed();
        let stats = ProcessStats {
            total_reads,
            processed_reads: processed,
            failed_reads: failed,
            total_bases,
            elapsed,
            reads_per_second: if elapsed.as_secs_f64() > 0.0 {
                processed as f64 / elapsed.as_secs_f64()
            } else {
                0.0
            },
            bases_per_second: if elapsed.as_secs_f64() > 0.0 {
                total_bases as f64 / elapsed.as_secs_f64()
            } else {
                0.0
            },
        };

        Ok(stats)
    }
}

impl Default for ProcessingPipeline {
    fn default() -> Self {
        Self::new(
            std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4),
            100,
            1000,
        )
    }
}
