use clap::Parser;
use nanopore_dtw::cli::{Cli, Commands, DtwMode, DistanceMetric};
use nanopore_dtw::{
    DtwAlgorithm, ProcessingConfig, print_stats, run_pipeline,
    find_files, create_reader,
};
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or(cli.log_level.to_filter())
    ).format_timestamp_millis().init();

    match cli.command {
        Commands::Align(args) => run_align(args),
        Commands::Index(args) => run_index(args),
        Commands::Stats(args) => run_stats(args),
        Commands::Convert(args) => run_convert(args),
        Commands::Bench(args) => run_bench(args),
    }
}

fn run_align(args: nanopore_dtw::cli::AlignArgs) -> Result<(), Box<dyn std::error::Error>> {
    let num_threads = if args.threads == 0 {
        std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(4)
    } else {
        args.threads
    };

    let dtw_algo = match args.dtw_mode {
        DtwMode::Standard => DtwAlgorithm::Standard,
        DtwMode::Fast => DtwAlgorithm::Fast,
        DtwMode::Banded => DtwAlgorithm::Banded,
    };

    let metric = match args.metric {
        DistanceMetric::L1 => nanopore_dtw::DistanceMetric::L1,
        DistanceMetric::L2 => nanopore_dtw::DistanceMetric::L2,
        DistanceMetric::Lp => nanopore_dtw::DistanceMetric::Lp(args.lp_p),
    };

    let config = ProcessingConfig {
        input_paths: args.input,
        reference_path: args.reference,
        output_path: args.output,
        kmer_model_path: args.model,
        kmer_size: args.kmer_size,
        num_threads,
        batch_size: args.batch_size,
        channel_capacity: args.channel_capacity,
        dtw_algorithm: dtw_algo,
        distance_metric: metric,
        band_width: args.band_width,
        max_distance: args.max_distance,
        downsample_factor: args.downsample,
        normalize: args.normalize,
        median_filter: args.median_filter,
        recursive: args.recursive,
        verbose: args.verbose,
        min_signal_length: args.min_signal_length,
        use_r9_model: args.r9_model,
    };

    let start = Instant::now();
    log::info!("Starting nanopore-dtw alignment pipeline");
    log::info!("Using {} worker threads", num_threads);
    log::info!("DTW algorithm: {:?}", dtw_algo);

    if args.tui {
        match nanopore_dtw::run_pipeline_with_tui(config) {
            Ok(stats) => {
                print_stats(&stats);
                log::info!("Pipeline completed successfully in {:?}", start.elapsed());
                Ok(())
            }
            Err(e) => {
                log::error!("Pipeline failed: {}", e);
                Err(e.into())
            }
        }
    } else {
        match run_pipeline(config) {
            Ok(stats) => {
                print_stats(&stats);
                log::info!("Pipeline completed successfully in {:?}", start.elapsed());
                Ok(())
            }
            Err(e) => {
                log::error!("Pipeline failed: {}", e);
                Err(e.into())
            }
        }
    }
}

fn run_index(args: nanopore_dtw::cli::IndexArgs) -> Result<(), Box<dyn std::error::Error>> {
    log::info!("Indexing reference: {:?}", args.reference);
    
    let mut reference = nanopore_dtw::ReferenceDictionary::new()
        .with_kmer_size(args.kmer_size);

    if let Some(model_path) = &args.model {
        let model = nanopore_dtw::KmerModel::load_from_file(model_path, args.kmer_size)?;
        reference = reference.with_kmer_model(model);
    }

    reference.load_from_fasta(&args.reference)?;
    
    let index_data = serde_json::to_vec(reference.sequences())?;
    std::fs::write(&args.output, index_data)?;
    
    println!("Reference index saved to {:?}", args.output);
    println!("Indexed {} sequences", reference.len());
    
    Ok(())
}

fn run_stats(args: nanopore_dtw::cli::StatsArgs) -> Result<(), Box<dyn std::error::Error>> {
    let extensions = ["fast5", "pod5"];
    let mut total_reads = 0usize;
    let mut total_samples = 0usize;
    let mut file_count = 0usize;

    for path in &args.input {
        if path.is_dir() {
            let files = find_files(path, &extensions, args.recursive)?;
            for file in files {
                match create_reader(&file) {
                    Ok(mut reader) => {
                        file_count += 1;
                        let reads = reader.len();
                        total_reads += reads;
                        
                        if args.verbose {
                            let batch = reader.read_batch(reads)?;
                            let samples: usize = batch.iter().map(|r| r.len()).sum();
                            total_samples += samples;
                            println!("{:?}: {} reads, {} samples", file, reads, samples);
                        }
                    }
                    Err(e) => {
                        log::warn!("Skipping {:?}: {}", file, e);
                    }
                }
            }
        } else if path.is_file() {
            match create_reader(path) {
                Ok(mut reader) => {
                    file_count += 1;
                    let reads = reader.len();
                    total_reads += reads;
                    
                    if args.verbose {
                        let batch = reader.read_batch(reads)?;
                        let samples: usize = batch.iter().map(|r| r.len()).sum();
                        total_samples += samples;
                        println!("{:?}: {} reads, {} samples", path, reads, samples);
                    }
                }
                Err(e) => {
                    log::warn!("Skipping {:?}: {}", path, e);
                }
            }
        }
    }

    println!("\n{}", "═".repeat(50));
    println!("{}", "STATISTICS SUMMARY".to_string());
    println!("{}", "═".repeat(50));
    println!("  Files processed: {}", file_count);
    println!("  Total reads:     {}", nanopore_dtw::utils::format_number(total_reads));
    if args.verbose {
        println!("  Total samples:   {}", nanopore_dtw::utils::format_number(total_samples));
    }
    println!("{}", "═".repeat(50));

    Ok(())
}

fn run_convert(_args: nanopore_dtw::cli::ConvertArgs) -> Result<(), Box<dyn std::error::Error>> {
    println!("Convert command - implementation placeholder");
    println!("This feature will be available in a future release.");
    Ok(())
}

fn run_bench(args: nanopore_dtw::cli::BenchArgs) -> Result<(), Box<dyn std::error::Error>> {
    use rand::SeedableRng;
    use rand_distr::{Normal, Distribution};

    println!("{}", "═".repeat(60));
    println!("{}", "DTW BENCHMARK".to_string());
    println!("{}", "═".repeat(60));

    let mut rng = rand::rngs::StdRng::seed_from_u64(42);
    let normal = Normal::new(0.0, 1.0).unwrap();

    let signal: Vec<f32> = (0..args.signal_length)
        .map(|_| normal.sample(&mut rng) as f32)
        .collect();
    
    let reference: Vec<f32> = (0..args.reference_length)
        .map(|_| normal.sample(&mut rng) as f32)
        .collect();

    let modes = if args.modes.is_empty() {
        vec![DtwMode::Standard, DtwMode::Fast, DtwMode::Banded]
    } else {
        args.modes.clone()
    };

    let band_widths = if args.band_widths.is_empty() {
        vec![50, 100, 200]
    } else {
        args.band_widths.clone()
    };

    for mode in &modes {
        let algo = match mode {
            DtwMode::Standard => DtwAlgorithm::Standard,
            DtwMode::Fast => DtwAlgorithm::Fast,
            DtwMode::Banded => DtwAlgorithm::Banded,
        };

        if *mode == DtwMode::Banded {
            for &bw in &band_widths {
                let config = nanopore_dtw::dtw::DtwConfig::default()
                    .with_band_width(bw);
                let aligner = nanopore_dtw::DtwAligner::new(config, algo);

                let start = Instant::now();
                let mut total_dist = 0.0;
                for _ in 0..args.iterations {
                    if let Ok(result) = aligner.align(&signal, &reference) {
                        total_dist += result.total_distance;
                    }
                }
                let elapsed = start.elapsed();
                let avg_time = elapsed / args.iterations as u32;

                println!("{:?} (band={}):", mode, bw);
                println!("  Total time:   {:?}", elapsed);
                println!("  Avg per iter: {:?}", avg_time);
                println!("  Avg dist:     {:.4}", total_dist / args.iterations as f32);
                println!();
            }
        } else {
            let config = nanopore_dtw::dtw::DtwConfig::default().without_band();
            let aligner = nanopore_dtw::DtwAligner::new(config, algo);

            let start = Instant::now();
            let mut total_dist = 0.0;
            for _ in 0..args.iterations {
                if let Ok(result) = aligner.align(&signal, &reference) {
                    total_dist += result.total_distance;
                }
            }
            let elapsed = start.elapsed();
            let avg_time = elapsed / args.iterations as u32;

            println!("{:?}:", mode);
            println!("  Total time:   {:?}", elapsed);
            println!("  Avg per iter: {:?}", avg_time);
            println!("  Avg dist:     {:.4}", total_dist / args.iterations as f32);
            println!();
        }
    }

    Ok(())
}
