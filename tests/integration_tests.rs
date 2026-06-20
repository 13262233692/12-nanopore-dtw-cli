use nanopore_dtw::*;
use std::path::PathBuf;
use tempfile::TempDir;

fn create_test_fasta(dir: &TempDir) -> PathBuf {
    let path = dir.path().join("test.fasta");
    let content = ">chr1
ATCGATCGATCGATCGATCGATCGATCGATCG
>chr2
GCTAGCTAGCTAGCTAGCTAGCTAGCTA
";
    std::fs::write(&path, content).unwrap();
    path
}

#[test]
fn test_load_reference_fasta() {
    let dir = TempDir::new().unwrap();
    let fasta_path = create_test_fasta(&dir);
    
    let mut reference = ReferenceDictionary::new().with_kmer_size(5);
    reference.load_from_fasta(&fasta_path).unwrap();
    
    assert_eq!(reference.len(), 2);
    assert!(reference.get("chr1").is_some());
    assert!(reference.get("chr2").is_some());
    
    let chr1 = reference.get("chr1").unwrap();
    assert_eq!(chr1.length, 32);
    assert_eq!(chr1.kmers.len(), 28);
}

#[test]
fn test_kmer_model_r9() {
    let model = KmerModel::load_r9_model(5);
    assert_eq!(model.len(), 1024);
    assert!(model.contains("AAAAA"));
    assert!(model.contains("TTTTT"));
    assert!(model.contains("GGGGG"));
    assert!(model.contains("CCCCC"));
    
    let (mean, std) = model.get_current("ATCG");
    assert!(mean > 0.0);
    assert!(std > 0.0);
}

#[test]
fn test_dtw_aligner_creation() {
    let config = DtwConfig::default();
    let aligner = DtwAligner::new(config, DtwAlgorithm::Banded);
    
    let signal: Vec<f32> = (0..100).map(|x| (x as f32 * 0.1).sin()).collect();
    let reference: Vec<f32> = (0..80).map(|x| (x as f32 * 0.12).sin()).collect();
    
    let result = aligner.align(&signal, &reference).unwrap();
    assert!(result.total_distance > 0.0);
    assert!(result.path_length > 0);
    assert_eq!(result.alignment_path.len(), result.path_length);
}

#[test]
fn test_dtw_modes() {
    let signal: Vec<f32> = (0..200).map(|x| (x as f32 * 0.05).sin() * 50.0 + 90.0).collect();
    let reference: Vec<f32> = (0..180).map(|x| (x as f32 * 0.06).sin() * 50.0 + 90.0).collect();
    
    let modes = [
        DtwAlgorithm::Standard,
        DtwAlgorithm::Fast,
        DtwAlgorithm::Banded,
    ];
    
    for mode in &modes {
        let config = if *mode == DtwAlgorithm::Standard {
            DtwConfig::default().without_band()
        } else {
            DtwConfig::default().with_band_width(50)
        };
        
        let aligner = DtwAligner::new(config, *mode);
        let result = aligner.align(&signal, &reference);
        
        assert!(result.is_ok(), "Mode {:?} should succeed", mode);
        let result = result.unwrap();
        assert!(result.total_distance > 0.0, "Mode {:?} should have distance > 0", mode);
        assert!(result.path_length > 0, "Mode {:?} should have path length > 0", mode);
    }
}

#[test]
fn test_signal_normalization() {
    let mut signal: Vec<f32> = vec![1.0, 2.0, 3.0, 4.0, 5.0];
    normalize_signal(&mut signal);
    
    let mean: f32 = signal.iter().sum::<f32>() / signal.len() as f32;
    let variance: f32 = signal.iter().map(|&x| (x - mean).powi(2)).sum::<f32>() / signal.len() as f32;
    let std = variance.sqrt();
    
    assert!(mean.abs() < 1e-6);
    assert!((std - 1.0).abs() < 1e-6);
}

#[test]
fn test_median_filter() {
    let signal = vec![1.0, 100.0, 3.0, 4.0, 100.0, 6.0];
    let filtered = median_filter(&signal, 3);
    
    assert_eq!(filtered.len(), signal.len());
    assert!(filtered[1] < 100.0, "Outlier should be filtered");
    assert!(filtered[4] < 100.0, "Outlier should be filtered");
}

#[test]
fn test_downsample() {
    let signal: Vec<f32> = (0..100).map(|x| x as f32).collect();
    let downsampled = downsample(&signal, 2);
    
    assert_eq!(downsampled.len(), 50);
    assert_eq!(downsampled[0], 0.5);
    assert_eq!(downsampled[1], 2.5);
}

#[test]
fn test_distance_metrics() {
    use dtw::core::DistanceMetric as CoreMetric;
    
    assert_eq!(distance(5.0, 3.0, CoreMetric::L1), 2.0);
    assert_eq!(distance(5.0, 3.0, CoreMetric::L2), 4.0);
    assert!((distance(5.0, 3.0, CoreMetric::Lp(3.0)) - 8.0).abs() < 1e-6);
}

#[test]
fn test_reverse_complement() {
    let seq = b"ATCGATCG";
    let rc = reverse_complement(seq);
    
    assert_eq!(rc, b"CGATCGAT");
}

#[test]
fn test_quality_score_calculation() {
    assert_eq!(calculate_quality_score(1.0), 0);
    assert!(calculate_quality_score(0.1) > 0);
    assert!(calculate_quality_score(0.0001) > 30);
}

#[test]
fn test_file_format_detection() {
    use types::FileFormat;
    
    assert_eq!(FileFormat::from_path("test.fast5"), FileFormat::Fast5);
    assert_eq!(FileFormat::from_path("test.FAST5"), FileFormat::Fast5);
    assert_eq!(FileFormat::from_path("test.pod5"), FileFormat::Pod5);
    assert_eq!(FileFormat::from_path("test.sam"), FileFormat::Sam);
    assert_eq!(FileFormat::from_path("test.bam"), FileFormat::Bam);
    assert_eq!(FileFormat::from_path("test.txt"), FileFormat::Unknown);
}

#[test]
fn test_process_stats_formatting() {
    let stats = types::ProcessStats {
        total_reads: 1000,
        processed_reads: 950,
        failed_reads: 50,
        total_bases: 1_000_000,
        elapsed: std::time::Duration::from_secs(10),
        reads_per_second: 95.0,
        bases_per_second: 100_000.0,
    };
    
    assert_eq!(stats.total_reads, 1000);
    assert_eq!(stats.processed_reads, 950);
    assert_eq!(stats.failed_reads, 50);
    assert_eq!(stats.total_bases, 1_000_000);
}

#[test]
fn test_alignment_info_default() {
    let aln = types::AlignmentInfo::default();
    assert_eq!(aln.read_id, "");
    assert_eq!(aln.flag, 0);
    assert_eq!(aln.reference, "*");
    assert_eq!(aln.position, -1);
    assert_eq!(aln.mapping_quality, 0);
    assert_eq!(aln.cigar, "*");
    assert_eq!(aln.sequence, "*");
    assert_eq!(aln.quality, "*");
}

#[test]
fn test_generate_kmers() {
    let seq = b"ATCGATCG";
    let kmers = generate_kmers(seq, 3);
    
    assert_eq!(kmers.len(), 6);
    assert_eq!(kmers[0], "ATC");
    assert_eq!(kmers[1], "TCG");
    assert_eq!(kmers[2], "CGA");
    assert_eq!(kmers[5], "TCG");
}

#[test]
fn test_dtw_short_signal() {
    let config = DtwConfig::default();
    let aligner = DtwAligner::new(config, DtwAlgorithm::Standard);
    
    let short_signal: Vec<f32> = vec![1.0];
    let reference: Vec<f32> = vec![1.0, 2.0, 3.0];
    
    let result = aligner.align(&short_signal, &reference);
    assert!(result.is_err());
}

#[test]
fn test_cost_matrix() {
    use dtw::core::CostMatrix;
    
    let mut matrix = CostMatrix::new(10, 10);
    matrix.set(0, 0, 42.0);
    assert_eq!(matrix.get(0, 0), 42.0);
    
    *matrix.get_mut(1, 1) = 100.0;
    assert_eq!(matrix.get(1, 1), 100.0);
}

#[test]
fn test_format_duration() {
    assert_eq!(format_duration(std::time::Duration::from_secs(5)), "5.00s");
    assert_eq!(format_duration(std::time::Duration::from_secs(65)), "1m05s");
    assert_eq!(format_duration(std::time::Duration::from_secs(3665)), "1h01m05s");
}

#[test]
fn test_format_number() {
    assert_eq!(format_number(1000), "1,000");
    assert_eq!(format_number(1000000), "1,000,000");
    assert_eq!(format_number(1234567), "1,234,567");
}
