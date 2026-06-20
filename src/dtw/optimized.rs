use crate::dtw::core::{distance, DtwConfig, DtwAlignmentResult, DtwPathPoint};
use crate::error::{NanoDtwError, Result};
use crate::types::DtwPathPoint as TypesDtwPathPoint;

pub struct FastDtw;

impl FastDtw {
    pub fn align(signal: &[f32], reference: &[f32], config: &DtwConfig) -> Result<DtwAlignmentResult> {
        let n = signal.len();
        let m = reference.len();
        let min_size = config.band_width.unwrap_or(100);

        if n.min(m) <= min_size {
            return Self::standard_dtw(signal, reference, config);
        }

        let shrink_factor = 2;
        let shrunken_signal = Self::downsample(signal, shrink_factor);
        let shrunken_reference = Self::downsample(reference, shrink_factor);

        let coarse_result = Self::align(&shrunken_signal, &shrunken_reference, config)?;
        let path = Self::expand_path(&coarse_result.alignment_path, shrink_factor, n, m);

        let band_width = config.band_width.unwrap_or(50);
        Self::constrained_dtw(signal, reference, config, &path, band_width)
    }

    fn standard_dtw(
        signal: &[f32],
        reference: &[f32],
        config: &DtwConfig,
    ) -> Result<DtwAlignmentResult> {
        let n = signal.len();
        let m = reference.len();

        let mut dtw = vec![vec![f32::INFINITY; m + 1]; n + 1];
        dtw[0][0] = 0.0;

        for i in 1..=n {
            for j in 1..=m {
                let cost = distance(signal[i - 1], reference[j - 1], config.metric);
                dtw[i][j] = cost + dtw[i - 1][j - 1].min(dtw[i - 1][j]).min(dtw[i][j - 1]);
            }
        }

        Self::extract_path(&dtw, n, m, n, m)
    }

    fn constrained_dtw(
        signal: &[f32],
        reference: &[f32],
        config: &DtwConfig,
        path: &[(usize, usize)],
        band_width: usize,
    ) -> Result<DtwAlignmentResult> {
        let n = signal.len();
        let m = reference.len();

        let mut min_col = vec![usize::MAX; n];
        let mut max_col = vec![0usize; n];

        for &(i, j) in path {
            if i < n {
                let low = j.saturating_sub(band_width);
                let high = (j + band_width).min(m - 1);
                min_col[i] = min_col[i].min(low);
                max_col[i] = max_col[i].max(high);
            }
        }

        let mut dtw = vec![vec![f32::INFINITY; m + 1]; n + 1];
        dtw[0][0] = 0.0;

        for i in 1..=n {
            let start_j = min_col[i - 1].saturating_sub(1) + 1;
            let end_j = max_col[i - 1].saturating_add(1).min(m);

            for j in start_j..=end_j {
                let cost = distance(signal[i - 1], reference[j - 1], config.metric);
                dtw[i][j] = cost + dtw[i - 1][j - 1].min(dtw[i - 1][j]).min(dtw[i][j - 1]);
            }
        }

        Self::extract_path(&dtw, n, m, n, m)
    }

    fn downsample(signal: &[f32], factor: usize) -> Vec<f32> {
        if factor <= 1 {
            return signal.to_vec();
        }

        let mut result = Vec::with_capacity(signal.len() / factor + 1);
        let mut i = 0;
        while i < signal.len() {
            let end = (i + factor).min(signal.len());
            let sum: f32 = signal[i..end].iter().sum();
            result.push(sum / (end - i) as f32);
            i += factor;
        }
        result
    }

    fn expand_path(
        path: &[TypesDtwPathPoint],
        factor: usize,
        orig_n: usize,
        orig_m: usize,
    ) -> Vec<(usize, usize)> {
        let mut expanded = Vec::with_capacity(path.len() * factor);
        for p in path {
            let start_i = p.signal_idx * factor;
            let start_j = p.reference_idx * factor;
            for di in 0..factor {
                for dj in 0..factor {
                    let i = start_i + di;
                    let j = start_j + dj;
                    if i < orig_n && j < orig_m {
                        expanded.push((i, j));
                    }
                }
            }
        }
        expanded
    }

    fn extract_path(
        dtw: &[Vec<f32>],
        n: usize,
        m: usize,
        signal_len: usize,
        ref_len: usize,
    ) -> Result<DtwAlignmentResult> {
        let mut path = Vec::new();
        let (mut i, mut j) = (n, m);

        while i > 0 && j > 0 {
            path.push(DtwPathPoint {
                signal_idx: i - 1,
                reference_idx: j - 1,
                distance: dtw[i][j],
            });

            let min_prev = dtw[i - 1][j - 1].min(dtw[i - 1][j]).min(dtw[i][j - 1]);

            if (dtw[i - 1][j - 1] - min_prev).abs() < 1e-9 {
                i -= 1;
                j -= 1;
            } else if (dtw[i - 1][j] - min_prev).abs() < 1e-9 {
                i -= 1;
            } else {
                j -= 1;
            }
        }

        path.reverse();

        let total_distance = dtw[n][m];
        let path_length = path.len();

        if path_length == 0 {
            return Err(NanoDtwError::DtwError("Empty alignment path".to_string()));
        }

        Ok(DtwAlignmentResult {
            total_distance,
            normalized_distance: total_distance / path_length as f32,
            path_length,
            alignment_path: path
                .iter()
                .map(|p| TypesDtwPathPoint {
                    signal_idx: p.signal_idx,
                    reference_idx: p.reference_idx,
                    distance: p.distance,
                })
                .collect(),
            signal_start: 0,
            signal_end: signal_len - 1,
            reference_start: 0,
            reference_end: ref_len - 1,
        })
    }
}
