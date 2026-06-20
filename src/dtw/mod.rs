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
        use crate::dtw::banded::MAX_SIGNAL_LENGTH;

        let n = signal.len();
        let m = reference.len();

        if n > MAX_SIGNAL_LENGTH {
            return Err(NanoDtwError::SignalTooLong(n, MAX_SIGNAL_LENGTH));
        }
        if m > MAX_SIGNAL_LENGTH {
            return Err(NanoDtwError::SignalTooLong(m, MAX_SIGNAL_LENGTH));
        }

        let mut prev_row = vec![f32::INFINITY; m + 1];
        let mut curr_row = vec![f32::INFINITY; m + 1];
        prev_row[0] = 0.0;

        let dist_fn: Box<dyn Fn(f32, f32) -> f32> = match config.metric {
            DistanceMetric::L1 => Box::new(|a: f32, b: f32| (a - b).abs()),
            DistanceMetric::L2 => Box::new(|a: f32, b: f32| (a - b).powi(2)),
            DistanceMetric::Lp(p) => Box::new(move |a: f32, b: f32| (a - b).abs().powf(p)),
        };

        let mut path_trace: Vec<Vec<u8>> = Vec::with_capacity(n);
        let mut row_bounds: Vec<(usize, usize)> = Vec::with_capacity(n);

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

            row_bounds.push((start_j, end_j));

            curr_row.fill(f32::INFINITY);
            if i == 1 {
                curr_row[0] = f32::INFINITY;
            }

            let mut row_trace = vec![0u8; m + 1];

            for j in start_j..=end_j {
                if j > m {
                    break;
                }

                let cost = dist_fn(signal[i - 1], reference[j - 1]);
                let cost = (config.window_fn)(cost, i - 1, j - 1);

                let diag_val = if j > 0 { prev_row[j - 1] } else { f32::INFINITY };
                let up_val = prev_row[j];
                let left_val = if j > 0 { curr_row[j - 1] } else { f32::INFINITY };

                let (min_val, direction) = if diag_val <= up_val && diag_val <= left_val {
                    (diag_val, 0b01)
                } else if up_val <= left_val {
                    (up_val, 0b10)
                } else {
                    (left_val, 0b11)
                };

                curr_row[j] = cost + min_val;
                row_trace[j] = direction;

                if let Some(max_dist) = config.max_distance {
                    if curr_row[j] > max_dist {
                        curr_row[j] = f32::INFINITY;
                    }
                }
            }

            path_trace.push(row_trace);
            std::mem::swap(&mut prev_row, &mut curr_row);
        }

        let final_distance = prev_row[m];

        let mut path = Vec::new();
        let (mut i, mut j) = (n, m);

        let max_iterations = (n + m) * 2;
        let mut iterations = 0;

        while i > 0 && j > 0 && iterations < max_iterations {
            iterations += 1;

            path.push(DtwPathPoint {
                signal_idx: i - 1,
                reference_idx: j - 1,
                distance: 0.0,
            });

            let trace_idx = i - 1;
            if trace_idx >= path_trace.len() {
                log::warn!("Standard DTW path trace index out of bounds: {} >= {}", trace_idx, path_trace.len());
                break;
            }

            let (start_j, end_j) = if trace_idx < row_bounds.len() {
                row_bounds[trace_idx]
            } else {
                (1, m)
            };

            let clamped_j = j.clamp(start_j, end_j);
            
            let row_trace = &path_trace[trace_idx];
            if clamped_j >= row_trace.len() {
                log::warn!("Standard DTW column index out of bounds: {} >= {}", clamped_j, row_trace.len());
                if i > 1 && j > 1 {
                    i -= 1;
                    j -= 1;
                } else if i > 1 {
                    i -= 1;
                } else if j > 1 {
                    j -= 1;
                } else {
                    break;
                }
                continue;
            }

            let direction = row_trace[clamped_j];
            match direction {
                0b01 => {
                    if i > 1 { i -= 1; }
                    if j > 1 { j -= 1; }
                }
                0b10 => {
                    if i > 1 { i -= 1; }
                }
                0b11 => {
                    if j > 1 { j -= 1; }
                }
                _ => {
                    if i > 1 && j > 1 {
                        i -= 1;
                        j -= 1;
                    } else if i > 1 {
                        i -= 1;
                    } else if j > 1 {
                        j -= 1;
                    } else {
                        break;
                    }
                }
            }
        }

        path.reverse();

        let path_length = path.len();

        if path_length == 0 {
            return Err(NanoDtwError::DtwError("Empty alignment path".to_string()));
        }

        Ok(DtwAlignmentResult {
            total_distance: final_distance,
            normalized_distance: final_distance / path_length as f32,
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
