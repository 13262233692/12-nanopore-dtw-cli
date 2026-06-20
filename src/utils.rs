use crate::error::Result;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub fn find_files<P: AsRef<Path>>(
    base_path: P,
    extensions: &[&str],
    recursive: bool,
) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    let walker = if recursive {
        WalkDir::new(base_path)
    } else {
        WalkDir::new(base_path).max_depth(1)
    };

    for entry in walker.into_iter().filter_map(|e| e.ok()) {
        if entry.file_type().is_file() {
            if let Some(ext) = entry.path().extension().and_then(|e| e.to_str()) {
                if extensions.iter().any(|&e| e.eq_ignore_ascii_case(ext)) {
                    files.push(entry.path().to_path_buf());
                }
            }
        }
    }

    Ok(files)
}

#[inline]
pub fn normalize_signal(signal: &mut [f32]) {
    if signal.is_empty() {
        return;
    }

    let mean = signal.iter().sum::<f32>() / signal.len() as f32;
    let variance = signal
        .iter()
        .map(|&x| (x - mean).powi(2))
        .sum::<f32>()
        / signal.len() as f32;
    let std = variance.sqrt();

    if std > 0.0 {
        for x in signal.iter_mut() {
            *x = (*x - mean) / std;
        }
    }
}

#[inline]
pub fn median_filter(signal: &[f32], window_size: usize) -> Vec<f32> {
    if window_size < 2 || signal.len() < window_size {
        return signal.to_vec();
    }

    let half = window_size / 2;
    let mut result = Vec::with_capacity(signal.len());

    for i in 0..signal.len() {
        let start = if i < half { 0 } else { i - half };
        let end = std::cmp::min(signal.len(), i + half + 1);
        let mut window: Vec<f32> = signal[start..end].to_vec();
        window.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
        result.push(window[window.len() / 2]);
    }

    result
}

#[inline]
pub fn downsample(signal: &[f32], factor: usize) -> Vec<f32> {
    if factor <= 1 || signal.is_empty() {
        return signal.to_vec();
    }

    let mut result = Vec::with_capacity(signal.len() / factor + 1);
    let mut i = 0;
    while i < signal.len() {
        let end = std::cmp::min(i + factor, signal.len());
        let chunk = &signal[i..end];
        let avg = chunk.iter().sum::<f32>() / chunk.len() as f32;
        result.push(avg);
        i += factor;
    }

    result
}

#[inline]
pub fn l1_distance(a: f32, b: f32) -> f32 {
    (a - b).abs()
}

#[inline]
pub fn l2_distance(a: f32, b: f32) -> f32 {
    (a - b).powi(2)
}

#[inline]
pub fn clip_value(val: f32, min: f32, max: f32) -> f32 {
    val.max(min).min(max)
}

pub fn format_duration(d: std::time::Duration) -> String {
    let secs = d.as_secs();
    if secs < 60 {
        format!("{}.{:02}s", secs, d.subsec_millis() / 10)
    } else if secs < 3600 {
        let mins = secs / 60;
        let secs = secs % 60;
        format!("{}m{:02}s", mins, secs)
    } else {
        let hours = secs / 3600;
        let mins = (secs % 3600) / 60;
        let secs = secs % 60;
        format!("{}h{:02}m{:02}s", hours, mins, secs)
    }
}

pub fn format_number(n: usize) -> String {
    let mut s = n.to_string();
    let mut i = s.len();
    while i > 3 {
        i -= 3;
        s.insert(i, ',');
    }
    s
}

pub fn calculate_quality_score(accuracy: f32) -> u8 {
    if accuracy <= 0.0 {
        return 0;
    }
    let q = -10.0 * accuracy.log10();
    q.max(0.0).min(60.0) as u8
}

#[inline]
pub fn complement_base(base: u8) -> u8 {
    match base {
        b'A' => b'T',
        b'T' => b'A',
        b'C' => b'G',
        b'G' => b'C',
        b'a' => b't',
        b't' => b'a',
        b'c' => b'g',
        b'g' => b'c',
        other => other,
    }
}

pub fn reverse_complement(seq: &[u8]) -> Vec<u8> {
    seq.iter().rev().map(|&b| complement_base(b)).collect()
}
