use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    Sam,
    Bam,
    Json,
    Table,
    None,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum DtwMode {
    Standard,
    Fast,
    Banded,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum DistanceMetric {
    L1,
    L2,
    Lp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

#[derive(Parser, Debug)]
#[command(
    name = "nanopore-dtw",
    version,
    about = "Nanopore sequencing raw signal processing with DTW alignment",
    long_about = "High-performance command-line tool for processing Oxford Nanopore \
                  Technologies sequencing data. Reads raw electrical signals from \
                  FAST5/POD5 files, performs dynamic time warping alignment against \
                  reference sequences, and outputs results in SAM/BAM format.",
    author = "Bioinformatics Team <team@biotools.dev>",
    help_template = "{before-help}{name} {version}
{author-with-newline}{about-with-newline}
{usage-heading} {usage}

{all-args}{after-help}"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,

    #[arg(short, long, value_enum, default_value_t = LogLevel::Info, global = true)]
    pub log_level: LogLevel,

    #[arg(long, global = true)]
    pub no_progress: bool,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Align raw signals to reference sequences using DTW
    Align(AlignArgs),

    /// Index reference sequences for faster alignment
    Index(IndexArgs),

    /// Show statistics about input files
    Stats(StatsArgs),

    /// Convert between file formats (FAST5/POD5)
    Convert(ConvertArgs),

    /// Benchmark DTW performance
    Bench(BenchArgs),
}

#[derive(Parser, Debug)]
pub struct AlignArgs {
    /// Input FAST5/POD5 files or directories
    #[arg(required = true, num_args = 1..)]
    pub input: Vec<PathBuf>,

    /// Reference FASTA file
    #[arg(short, long)]
    pub reference: PathBuf,

    /// Output file (SAM/BAM format, detected by extension)
    #[arg(short, long)]
    pub output: Option<PathBuf>,

    /// Output format (overrides file extension detection)
    #[arg(short = 'f', long, value_enum)]
    pub output_format: Option<OutputFormat>,

    /// K-mer model file
    #[arg(long)]
    pub model: Option<PathBuf>,

    /// K-mer size for signal alignment
    #[arg(long, default_value_t = 5)]
    pub kmer_size: usize,

    /// Number of worker threads
    #[arg(short = 't', long, default_value_t = 0)]
    pub threads: usize,

    /// Batch size for reading signals
    #[arg(long, default_value_t = 100)]
    pub batch_size: usize,

    /// Channel capacity for thread communication
    #[arg(long, default_value_t = 1000)]
    pub channel_capacity: usize,

    /// DTW algorithm mode
    #[arg(long, value_enum, default_value_t = DtwMode::Banded)]
    pub dtw_mode: DtwMode,

    /// DTW distance metric
    #[arg(long, value_enum, default_value_t = DistanceMetric::L2)]
    pub metric: DistanceMetric,

    /// Sakoe-Chiba band width for constrained DTW
    #[arg(long, default_value_t = 100)]
    pub band_width: usize,

    /// Lp norm parameter for Lp distance
    #[arg(long, default_value_t = 2.0)]
    pub lp_p: f32,

    /// Maximum DTW distance (early termination)
    #[arg(long)]
    pub max_distance: Option<f32>,

    /// Downsample factor for signals
    #[arg(long, default_value_t = 1)]
    pub downsample: usize,

    /// Normalize signals before alignment
    #[arg(long, default_value_t = true)]
    pub normalize: bool,

    /// Median filter window size (0 to disable)
    #[arg(long, default_value_t = 0)]
    pub median_filter: usize,

    /// Search recursively in input directories
    #[arg(short = 'R', long, default_value_t = true)]
    pub recursive: bool,

    /// Print detailed alignment information
    #[arg(short = 'v', long)]
    pub verbose: bool,

    /// Minimum signal length to process
    #[arg(long, default_value_t = 50)]
    pub min_signal_length: usize,

    /// Use built-in R9 kmer model
    #[arg(long)]
    pub r9_model: bool,

    /// Enable interactive TUI mode
    #[arg(long)]
    pub tui: bool,
}

#[derive(Parser, Debug)]
pub struct IndexArgs {
    /// Reference FASTA file
    #[arg(short, long)]
    pub reference: PathBuf,

    /// Output index file
    #[arg(short, long)]
    pub output: PathBuf,

    /// K-mer size
    #[arg(long, default_value_t = 5)]
    pub kmer_size: usize,

    /// K-mer model file
    #[arg(long)]
    pub model: Option<PathBuf>,
}

#[derive(Parser, Debug)]
pub struct StatsArgs {
    /// Input FAST5/POD5 files or directories
    #[arg(required = true, num_args = 1..)]
    pub input: Vec<PathBuf>,

    /// Search recursively in directories
    #[arg(short = 'r', long, default_value_t = true)]
    pub recursive: bool,

    /// Output detailed statistics
    #[arg(short = 'v', long)]
    pub verbose: bool,
}

#[derive(Parser, Debug)]
pub struct ConvertArgs {
    /// Input files
    #[arg(required = true, num_args = 1..)]
    pub input: Vec<PathBuf>,

    /// Output directory
    #[arg(short, long)]
    pub output: PathBuf,

    /// Output format
    #[arg(short = 'f', long, value_enum, default_value_t = OutputFormat::Sam)]
    pub format: OutputFormat,

    /// Search recursively
    #[arg(short = 'R', long, default_value_t = true)]
    pub recursive: bool,
}

#[derive(Parser, Debug)]
pub struct BenchArgs {
    /// Input FAST5 file for benchmarking
    #[arg(short, long)]
    pub input: Option<PathBuf>,

    /// Signal length for synthetic benchmark
    #[arg(long, default_value_t = 1000)]
    pub signal_length: usize,

    /// Reference length for synthetic benchmark
    #[arg(long, default_value_t = 1000)]
    pub reference_length: usize,

    /// Number of iterations
    #[arg(short = 'n', long, default_value_t = 100)]
    pub iterations: usize,

    /// DTW modes to benchmark
    #[arg(long, value_enum, num_args = 1..)]
    pub modes: Vec<DtwMode>,

    /// Band widths to test
    #[arg(long, num_args = 1..)]
    pub band_widths: Vec<usize>,
}

impl LogLevel {
    pub fn to_filter(&self) -> &'static str {
        match self {
            LogLevel::Error => "error",
            LogLevel::Warn => "warn",
            LogLevel::Info => "info",
            LogLevel::Debug => "debug",
            LogLevel::Trace => "trace",
        }
    }
}
