pub mod core;
pub mod optimized;
pub mod banded;

pub use core::{DtwAlignmentResult, DtwPathPoint, DtwConfig, DistanceMetric};
pub use optimized::FastDtw;
pub use banded::BandedDtw;

use crate::error::{NanoDtwError, Result};
use crate::types::DtwPathPoint as TypesDtwPathPoint;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DtwAlgorithm {
    Standard,
    Fast,
    Banded,
}

#[derive(Clone)]
pub struct DtwAligner {
    config: DtwConfig,
    algorithm: DtwAlgorithm,
}

impl DtwAligner {
    pub fn new(config: DtwConfig, algorithm: DtwAlgorithm) -> Self {
        Self { config, algorithm }
    }

    pub fn align(&self, signal: &[f32], reference: &[f32]) -> Result<DtwAlignmentResult> {
        if signal.len() < 2 {
            return Err(NanoDtwError::SignalTooShort(signal.len(), 2));
        }
        if reference.len() < 2 {
            return Err(NanoDtwError::SignalTooShort(reference.len(), 2));
        }

        match self.algorithm {
            DtwAlgorithm::Standard => Self::standard_dtw(signal, reference, &self.config),
            DtwAlgorithm::Fast => FastDtw::align(signal, reference, &self.config),
            DtwAlgorithm::Banded => BandedDtw::align(signal, reference, &self.config),
        }
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

        let dist_fn: Box<dyn Fn(f32, f32) -> f32> = match config.metric {
            DistanceMetric::L1 => Box::new(|a: f32, b: f32| (a - b).abs()),
            DistanceMetric::L2 => Box::new(|a: f32, b: f32| (a - b).powi(2)),
            DistanceMetric::Lp(p) => Box::new(move |a: f32, b: f32| (a - b).abs().powf(p)),
        };

        for i in 1..=n {
            let start_j = if let Some(bw) = config.band_width {
                (i as isize - bw as isize).max(1) as usize
            } else {
                1
            };
            let end_j = if let Some(bw) = config.band_width {
                (i + bw).min(m)
            } else {
                m
            };

            for j in start_j..=end_j {
                let cost = dist_fn(signal[i - 1], reference[j - 1]);
                let cost = (config.window_fn)(cost, i - 1, j - 1);

                dtw[i][j] = cost + dtw[i - 1][j - 1].min(dtw[i - 1][j]).min(dtw[i][j - 1]);
            }
        }

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
            signal_end: n - 1,
            reference_start: 0,
            reference_end: m - 1,
        })
    }
}

impl Default for DtwAligner {
    fn default() -> Self {
        Self {
            config: DtwConfig::default(),
            algorithm: DtwAlgorithm::Banded,
        }
    }
}
