use crate::error::Result;
use crate::io::SignalReader;
use crate::reference::ReferenceDictionary;
use crate::sync::Semaphore;
use crate::threading::WorkerPool;
use crate::tui::state::{LogLevel, TuiState};
use crate::types::{DtwResult, ProcessStats, RawSignal};
use crossbeam_channel::bounded;
use indicatif::{ProgressBar, ProgressStyle};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Instant;

pub const DEFAULT_MAX_OPEN_FILES: usize = 32;
pub const DEFAULT_SIGNAL_QUEUE_DEPTH: usize = 1000;

pub struct ProcessingPipeline {
    num_workers: usize,
    batch_size: usize,
    channel_capacity: usize,
    max_open_files: usize,
    max_signal_queue_depth: usize,
}

impl ProcessingPipeline {
    pub fn new(num_workers: usize, batch_size: usize, channel_capacity: usize) -> Self {
        Self {
            num_workers,
            batch_size,
            channel_capacity,
            max_open_files: DEFAULT_MAX_OPEN_FILES,
            max_signal_queue_depth: DEFAULT_SIGNAL_QUEUE_DEPTH,
        }
    }

    pub fn with_max_open_files(mut self, max: usize) -> Self {
        self.max_open_files = max.max(1);
        self
    }

    pub fn with_max_signal_queue_depth(mut self, max: usize) -> Self {
        self.max_signal_queue_depth = max.max(100);
        self
    }

    pub fn run_with_paths(
        &self,
        input_paths: Vec<PathBuf>,
        reference: ReferenceDictionary,
        recursive: bool,
        mut output_handler: impl FnMut(DtwResult) -> Result<()>,
    ) -> Result<ProcessStats> {
        let extensions = ["fast5", "pod5"];
        let mut file_paths = Vec::new();

        for path in &input_paths {
            if path.is_dir() {
                let files = crate::utils::find_files(path, &extensions, recursive)?;
                file_paths.extend(files);
            } else {
                file_paths.push(path.clone());
            }
        }

        if file_paths.is_empty() {
            return Err(crate::error::NanoDtwError::NoData);
        }

        log::info!("Found {} files to process", file_paths.len());

        let mut total_reads = 0usize;
        {
            let semaphore = Semaphore::new(self.max_open_files);
            for path in &file_paths {
                let _guard = semaphore.acquire();
                match crate::io::create_reader(path) {
                    Ok(reader) => {
                        total_reads += reader.len();
                    }
                    Err(e) => {
                        log::warn!("Failed to open file {}: {}", path.display(), e);
                    }
                }
            }
        }

        log::info!("Found {} reads across {} files", total_reads, file_paths.len());

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
        let failed = 0usize;
        let mut total_bases = 0usize;

        let semaphore = Arc::new(Semaphore::new(self.max_open_files));
        let signal_semaphore = Semaphore::new(self.max_signal_queue_depth);

        let file_paths_clone = file_paths.clone();
        let batch_size = self.batch_size;
        let signal_tx_clone = signal_tx.clone();
        let signal_semaphore_clone = signal_semaphore.clone();
        let semaphore_clone = semaphore.clone();

        let reader_thread = std::thread::Builder::new()
            .name("file-reader".to_string())
            .spawn(move || {
                for path in &file_paths_clone {
                    let _file_guard = semaphore_clone.acquire();
                    
                    let reader = match crate::io::create_reader(path) {
                        Ok(r) => r,
                        Err(e) => {
                            log::warn!("Failed to open file {}: {}", path.display(), e);
                            continue;
                        }
                    };

                    let mut reader = reader;
                    loop {
                        let batch = match reader.read_batch(batch_size) {
                            Ok(b) => b,
                            Err(e) => {
                                log::warn!("Error reading from {}: {}", path.display(), e);
                                break;
                            }
                        };

                        if batch.is_empty() {
                            break;
                        }

                        for signal in batch {
                            let _queue_guard = signal_semaphore_clone.acquire();
                            
                            if signal.samples.is_empty() {
                                continue;
                            }

                            if let Err(e) = signal_tx_clone.send(signal) {
                                log::warn!("Failed to send signal: {}", e);
                                break;
                            }
                        }
                    }
                }
            })?;

        drop(signal_tx);

        while let Ok(result) = result_rx.recv() {
            total_bases += result.mapped_sequence.len();
            processed += 1;
            pb.inc(1);
            pb.set_message(format!(
                "{} bases, {} failed, {} files",
                crate::utils::format_number(total_bases),
                failed,
                file_paths.len()
            ));

            if let Err(e) = output_handler(result) {
                log::warn!("Output handler error: {}", e);
            }

            if processed + failed >= total_reads {
                break;
            }
        }

        reader_thread.join().map_err(|e| {
            crate::error::NanoDtwError::ThreadError(format!(
                "Reader thread panicked: {:?}",
                e
            ))
        })?;

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
        let signal_semaphore = Semaphore::new(self.max_signal_queue_depth);

        for reader in readers.iter_mut() {
            loop {
                let batch = reader.read_batch(self.batch_size)?;
                if batch.is_empty() {
                    break;
                }

                for signal in batch {
                    let _queue_guard = signal_semaphore.acquire();

                    if signal.samples.is_empty() {
                        failed += 1;
                        pb.inc(1);
                        continue;
                    }

                    if let Err(e) = signal_tx.send(signal) {
                        log::warn!("Failed to send signal: {}", e);
                        break;
                    }
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

    pub fn run_with_paths_tui(
        &self,
        input_paths: Vec<PathBuf>,
        reference: ReferenceDictionary,
        recursive: bool,
        tui_state: TuiState,
        mut output_handler: impl FnMut(DtwResult) -> Result<()>,
    ) -> Result<ProcessStats> {
        let extensions = ["fast5", "pod5"];
        let mut file_paths = Vec::new();

        for path in &input_paths {
            if path.is_dir() {
                let files = crate::utils::find_files(path, &extensions, recursive)?;
                file_paths.extend(files);
            } else {
                file_paths.push(path.clone());
            }
        }

        if file_paths.is_empty() {
            return Err(crate::error::NanoDtwError::NoData);
        }

        tui_state.add_log(LogLevel::Info, format!("Found {} files to process", file_paths.len()));

        let mut total_reads = 0usize;
        {
            let semaphore = Semaphore::new(self.max_open_files);
            for path in &file_paths {
                let _guard = semaphore.acquire();
                match crate::io::create_reader(path) {
                    Ok(reader) => {
                        total_reads += reader.len();
                    }
                    Err(e) => {
                        tui_state.add_log(LogLevel::Warn, format!("Failed to open file {}: {}", path.display(), e));
                    }
                }
            }
        }

        tui_state.add_log(LogLevel::Info, format!("Found {} reads across {} files", total_reads, file_paths.len()));

        let (signal_tx, signal_rx) = bounded::<RawSignal>(self.channel_capacity);
        let (result_tx, result_rx) = bounded::<DtwResult>(self.channel_capacity);
        let queue_depth_tx = signal_tx.clone();

        let reference_arc = Arc::new(reference);
        let aligner = crate::dtw::DtwAligner::default();

        let pool = WorkerPool::new(
            self.num_workers,
            Arc::clone(&reference_arc),
            signal_rx,
            result_tx,
            aligner,
        )?;

        let start_time = Instant::now();
        let mut processed = 0usize;
        let failed = 0usize;
        let mut total_bases = 0usize;

        let semaphore = Arc::new(Semaphore::new(self.max_open_files));
        let signal_semaphore = Semaphore::new(self.max_signal_queue_depth);

        let file_paths_clone = file_paths.clone();
        let batch_size = self.batch_size;
        let signal_tx_clone = signal_tx.clone();
        let signal_semaphore_clone = signal_semaphore.clone();
        let semaphore_clone = semaphore.clone();
        let tui_state_clone = tui_state.clone();

        let reader_thread = std::thread::Builder::new()
            .name("file-reader".to_string())
            .spawn(move || {
                let mut active = 0usize;
                for path in &file_paths_clone {
                    let _file_guard = semaphore_clone.acquire();
                    active += 1;
                    tui_state_clone.set_active_files(active);

                    let reader = match crate::io::create_reader(path) {
                        Ok(r) => r,
                        Err(e) => {
                            tui_state_clone.add_log(LogLevel::Warn, format!("Failed to open file {}: {}", path.display(), e));
                            active = active.saturating_sub(1);
                            tui_state_clone.set_active_files(active);
                            continue;
                        }
                    };

                    let mut reader = reader;
                    loop {
                        while tui_state_clone.is_paused() {
                            std::thread::sleep(std::time::Duration::from_millis(50));
                        }

                        let batch = match reader.read_batch(batch_size) {
                            Ok(b) => b,
                            Err(e) => {
                                tui_state_clone.add_log(LogLevel::Warn, format!("Error reading from {}: {}", path.display(), e));
                                break;
                            }
                        };

                        if batch.is_empty() {
                            break;
                        }

                        for signal in batch {
                            let _queue_guard = signal_semaphore_clone.acquire();

                            if signal.samples.is_empty() {
                                continue;
                            }

                            if let Err(e) = signal_tx_clone.send(signal) {
                                tui_state_clone.add_log(LogLevel::Warn, format!("Failed to send signal: {}", e));
                                break;
                            }
                        }
                    }

                    active = active.saturating_sub(1);
                    tui_state_clone.set_active_files(active);
                }
            })?;

        drop(signal_tx);

        while let Ok(result) = result_rx.recv() {
            while tui_state.is_paused() {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }

            total_bases += result.mapped_sequence.len();
            processed += 1;
            tui_state.update_processed(processed, total_bases);
            tui_state.set_queue_depth(queue_depth_tx.len());

            if let Err(e) = output_handler(result) {
                tui_state.add_log(LogLevel::Warn, format!("Output handler error: {}", e));
            }

            if processed + failed >= total_reads {
                break;
            }
        }

        reader_thread.join().map_err(|e| {
            crate::error::NanoDtwError::ThreadError(format!(
                "Reader thread panicked: {:?}",
                e
            ))
        })?;

        pool.join();

        while let Ok(result) = result_rx.try_recv() {
            total_bases += result.mapped_sequence.len();
            processed += 1;
            tui_state.update_processed(processed, total_bases);
            if let Err(e) = output_handler(result) {
                tui_state.add_log(LogLevel::Warn, format!("Output handler error: {}", e));
            }
        }

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pipeline_config() {
        let pipeline = ProcessingPipeline::new(4, 100, 1000)
            .with_max_open_files(16)
            .with_max_signal_queue_depth(500);
        
        assert_eq!(pipeline.max_open_files, 16);
        assert_eq!(pipeline.max_signal_queue_depth, 500);
    }

    #[test]
    fn test_pipeline_config_min_values() {
        let pipeline = ProcessingPipeline::new(4, 100, 1000)
            .with_max_open_files(0)
            .with_max_signal_queue_depth(0);
        
        assert_eq!(pipeline.max_open_files, 1);
        assert_eq!(pipeline.max_signal_queue_depth, 100);
    }
}
