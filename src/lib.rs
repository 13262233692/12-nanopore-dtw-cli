//! Nanopore DTW - High-performance raw signal alignment for nanopore sequencing
//!
//! This library provides core functionality for processing Oxford Nanopore
//! Technologies sequencing data, including HDF5 file reading, multi-threaded
//! signal processing, and dynamic time warping alignment.

pub mod cli;
pub mod dtw;
pub mod error;
pub mod io;
pub mod reference;
pub mod sync;
pub mod threading;
pub mod types;
pub mod utils;

pub use error::{NanoDtwError, Result};
pub use types::*;
pub use dtw::{DtwAligner, DtwAlgorithm, DtwConfig, DistanceMetric};
pub use dtw::core::distance;
pub use io::{create_reader, AlignmentWriter, SamBamWriter, SignalReader};
#[cfg(feature = "hdf5")]
pub use io::{Fast5Reader, Pod5Reader};
pub use reference::{KmerModel, ReferenceDictionary, load_fasta, generate_kmers};
pub use threading::{ProcessingPipeline, WorkerPool};
pub use utils::*;

use std::path::PathBuf;

pub struct ProcessingConfig {
    pub input_paths: Vec<PathBuf>,
    pub reference_path: PathBuf,
    pub output_path: Option<PathBuf>,
    pub kmer_model_path: Option<PathBuf>,
    pub kmer_size: usize,
    pub num_threads: usize,
    pub batch_size: usize,
    pub channel_capacity: usize,
    pub dtw_algorithm: DtwAlgorithm,
    pub distance_metric: DistanceMetric,
    pub band_width: usize,
    pub max_distance: Option<f32>,
    pub downsample_factor: usize,
    pub normalize: bool,
    pub median_filter: usize,
    pub recursive: bool,
    pub verbose: bool,
    pub min_signal_length: usize,
    pub use_r9_model: bool,
}

impl Default for ProcessingConfig {
    fn default() -> Self {
        Self {
            input_paths: Vec::new(),
            reference_path: PathBuf::new(),
            output_path: None,
            kmer_model_path: None,
            kmer_size: 5,
            num_threads: std::thread::available_parallelism()
                .map(|n| n.get())
                .unwrap_or(4),
            batch_size: 100,
            channel_capacity: 1000,
            dtw_algorithm: DtwAlgorithm::Banded,
            distance_metric: DistanceMetric::L2,
            band_width: 100,
            max_distance: None,
            downsample_factor: 1,
            normalize: true,
            median_filter: 0,
            recursive: true,
            verbose: false,
            min_signal_length: 50,
            use_r9_model: false,
        }
    }
}

pub fn run_pipeline(config: ProcessingConfig) -> Result<ProcessStats> {
    use crate::dtw::core::DistanceMetric as CoreMetric;
    use crate::threading::pipeline::DEFAULT_MAX_OPEN_FILES;
    use crate::threading::pipeline::DEFAULT_SIGNAL_QUEUE_DEPTH;

    let mut reference = ReferenceDictionary::new()
        .with_kmer_size(config.kmer_size);

    if config.use_r9_model {
        let model = KmerModel::load_r9_model(config.kmer_size);
        reference = reference.with_kmer_model(model);
    } else if let Some(model_path) = &config.kmer_model_path {
        let model = KmerModel::load_from_file(model_path, config.kmer_size)?;
        reference = reference.with_kmer_model(model);
    }

    reference.load_from_fasta(&config.reference_path)?;
    log::info!("Loaded {} reference sequences", reference.len());

    let _output_format = config.output_path.as_ref()
        .map(|p| FileFormat::from_path(p))
        .unwrap_or(FileFormat::Sam);

    let mut writer = if let Some(out_path) = &config.output_path {
        SamBamWriter::new(out_path)?
    } else if atty::is(atty::Stream::Stdout) {
        log::info!("Writing to stdout in SAM format");
        SamBamWriter::stdout(FileFormat::Sam)?
    } else {
        SamBamWriter::stdout(FileFormat::Sam)?
    };

    writer.write_header(reference.sequences())?;

    let mut dtw_config = DtwConfig::default()
        .with_band_width(config.band_width)
        .with_metric(match config.distance_metric {
            DistanceMetric::L1 => CoreMetric::L1,
            DistanceMetric::L2 => CoreMetric::L2,
            DistanceMetric::Lp(_) => CoreMetric::Lp(2.0),
        });

    if let Some(max_dist) = config.max_distance {
        dtw_config = dtw_config.with_max_distance(max_dist);
    }

    let _aligner = DtwAligner::new(dtw_config, config.dtw_algorithm);
    let pipeline = ProcessingPipeline::new(
        config.num_threads,
        config.batch_size,
        config.channel_capacity,
    )
    .with_max_open_files(DEFAULT_MAX_OPEN_FILES)
    .with_max_signal_queue_depth(DEFAULT_SIGNAL_QUEUE_DEPTH);

    let verbose = config.verbose;
    let _normalize = config.normalize;
    let _median_filter = config.median_filter;
    let _downsample = config.downsample_factor;
    let _min_len = config.min_signal_length;
    let recursive = config.recursive;

    let input_paths = config.input_paths.clone();
    let stats = pipeline.run_with_paths(
        input_paths,
        reference,
        recursive,
        |result| {
            if verbose {
                println!(
                    "Read: {} | Ref: {} | Dist: {:.4} | MapQ: {} | Seq: {}...",
                    result.read_id,
                    result.reference_id,
                    result.normalized_distance,
                    result.mapping_quality,
                    result.mapped_sequence.chars().take(20).collect::<String>()
                );
            }
            writer.write_dtw_result(&result)?;
            Ok(())
        },
    )?;

    writer.flush()?;

    Ok(stats)
}

pub fn print_stats(stats: &ProcessStats) {
    println!("\n{}", "═".repeat(60));
    println!("{}", "PROCESSING SUMMARY".to_string());
    println!("{}", "═".repeat(60));
    println!("  Total reads:         {}", crate::utils::format_number(stats.total_reads));
    println!("  Processed reads:     {}", crate::utils::format_number(stats.processed_reads));
    println!("  Failed reads:        {}", crate::utils::format_number(stats.failed_reads));
    println!("  Total bases:         {}", crate::utils::format_number(stats.total_bases));
    println!("  Elapsed time:        {}", crate::utils::format_duration(stats.elapsed));
    println!("  Throughput:          {:.2} reads/s", stats.reads_per_second);
    println!("  Throughput:          {:.2} bases/s", stats.bases_per_second);
    
    if stats.total_reads > 0 {
        let success_rate = stats.processed_reads as f64 / stats.total_reads as f64 * 100.0;
        println!("  Success rate:        {:.2}%", success_rate);
    }
    println!("{}", "═".repeat(60));
}
