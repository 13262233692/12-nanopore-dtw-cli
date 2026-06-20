use criterion::{criterion_group, criterion_main, Criterion, BenchmarkId};
use nanopore_dtw::*;
use rand::{Rng, SeedableRng};
use rand_distr::{Normal, Distribution};

fn generate_signals(signal_len: usize, ref_len: usize, noise: f32) -> (Vec<f32>, Vec<f32>) {
    let mut rng = rand::rngs::StdRng::seed_from_u64(42);
    let normal = Normal::new(0.0, noise as f64).unwrap();
    
    let base_signal: Vec<f32> = (0..ref_len)
        .map(|i| ((i as f32 * 0.02).sin() * 30.0 + 90.0 + normal.sample(&mut rng) as f32))
        .collect();
    
    let mut signal = Vec::with_capacity(signal_len);
    for i in 0..signal_len {
        let idx = (i as f32 * ref_len as f32 / signal_len as f32) as usize;
        let idx = idx.min(ref_len - 1);
        signal.push(base_signal[idx] + normal.sample(&mut rng) as f32);
    }
    
    (signal, base_signal)
}

fn bench_dtw_algorithms(c: &mut Criterion) {
    let sizes = vec![
        (100, 100),
        (500, 500),
        (1000, 1000),
        (2000, 2000),
    ];
    
    let mut group = c.benchmark_group("DTW Algorithms");
    group.sample_size(20);
    group.measurement_time(std::time::Duration::from_secs(10));
    
    for (signal_len, ref_len) in sizes {
        let (signal, reference) = generate_signals(signal_len, ref_len, 1.0);
        
        group.bench_with_input(
            BenchmarkId::new("Standard", format!("{}_{}", signal_len, ref_len)),
            &(signal.clone(), reference.clone()),
            |b, (s, r)| {
                let config = DtwConfig::default().without_band();
                let aligner = DtwAligner::new(config, DtwAlgorithm::Standard);
                b.iter(|| aligner.align(s, r).unwrap());
            },
        );
        
        group.bench_with_input(
            BenchmarkId::new("Fast", format!("{}_{}", signal_len, ref_len)),
            &(signal.clone(), reference.clone()),
            |b, (s, r)| {
                let config = DtwConfig::default().with_band_width(100);
                let aligner = DtwAligner::new(config, DtwAlgorithm::Fast);
                b.iter(|| aligner.align(s, r).unwrap());
            },
        );
        
        for bw in [50, 100, 200] {
            group.bench_with_input(
                BenchmarkId::new(format!("Banded_bw{}", bw), format!("{}_{}", signal_len, ref_len)),
                &(signal.clone(), reference.clone()),
                |b, (s, r)| {
                    let config = DtwConfig::default().with_band_width(bw);
                    let aligner = DtwAligner::new(config, DtwAlgorithm::Banded);
                    b.iter(|| aligner.align(s, r).unwrap());
                },
            );
        }
    }
    
    group.finish();
}

fn bench_distance_metrics(c: &mut Criterion) {
    let mut rng = rand::rngs::StdRng::seed_from_u64(42);
    let normal = Normal::new(0.0, 1.0).unwrap();
    
    let values: Vec<f32> = (0..10_000)
        .map(|_| normal.sample(&mut rng) as f32)
        .collect();
    
    let mut group = c.benchmark_group("Distance Metrics");
    group.throughput(criterion::Throughput::Elements(10_000));
    
    group.bench_function("L1 Distance", |b| {
        b.iter(|| {
            let mut sum = 0.0;
            for i in 0..values.len() - 1 {
                sum += l1_distance(values[i], values[i + 1]);
            }
            sum
        });
    });
    
    group.bench_function("L2 Distance", |b| {
        b.iter(|| {
            let mut sum = 0.0;
            for i in 0..values.len() - 1 {
                sum += l2_distance(values[i], values[i + 1]);
            }
            sum
        });
    });
    
    group.finish();
}

fn bench_signal_processing(c: &mut Criterion) {
    let mut rng = rand::rngs::StdRng::seed_from_u64(42);
    let normal = Normal::new(90.0, 10.0).unwrap();
    
    let signal: Vec<f32> = (0..100_000)
        .map(|_| normal.sample(&mut rng) as f32)
        .collect();
    
    let mut group = c.benchmark_group("Signal Processing");
    group.throughput(criterion::Throughput::Elements(100_000));
    
    group.bench_function("Normalize Signal", |b| {
        b.iter_batched(
            || signal.clone(),
            |mut s| {
                normalize_signal(&mut s);
                s
            },
            criterion::BatchSize::LargeInput,
        );
    });
    
    for ws in [3, 5, 7] {
        group.bench_function(format!("Median Filter (window={})", ws), |b| {
            b.iter(|| median_filter(&signal, ws));
        });
    }
    
    for factor in [2, 4, 8] {
        group.bench_function(format!("Downsample (factor={})", factor), |b| {
            b.iter(|| downsample(&signal, factor));
        });
    }
    
    group.finish();
}

fn bench_kmer_generation(c: &mut Criterion) {
    let bases = ['A', 'T', 'C', 'G'];
    let mut rng = rand::rngs::StdRng::seed_from_u64(42);
    
    let seq: Vec<u8> = (0..10_000)
        .map(|_| bases[rng.gen_range(0..4)] as u8)
        .collect();
    
    let mut group = c.benchmark_group("K-mer Generation");
    
    for k in [3, 5, 7] {
        group.bench_with_input(
            BenchmarkId::from_parameter(k),
            &k,
            |b, &k| b.iter(|| generate_kmers(&seq, k)),
        );
    }
    
    group.finish();
}

criterion_group!(
    name = benches;
    config = Criterion::default().with_output_color(true);
    targets =
        bench_dtw_algorithms,
        bench_distance_metrics,
        bench_signal_processing,
        bench_kmer_generation,
);

criterion_main!(benches);
