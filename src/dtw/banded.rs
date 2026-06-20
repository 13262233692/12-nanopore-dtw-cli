use crate::dtw::core::{distance, DtwConfig, DtwAlignmentResult, DtwPathPoint};
use crate::error::{NanoDtwError, Result};
use crate::types::DtwPathPoint as TypesDtwPathPoint;

pub const MAX_SIGNAL_LENGTH: usize = 1_000_000;
pub const SAFETY_WINDOW_MARGIN: usize = 10;

pub struct BandedDtw;

impl BandedDtw {
    pub fn align(
        signal: &[f32],
        reference: &[f32],
        config: &DtwConfig,
    ) -> Result<DtwAlignmentResult> {
        let n = signal.len();
        let m = reference.len();

        if n > MAX_SIGNAL_LENGTH {
            return Err(NanoDtwError::SignalTooLong(n, MAX_SIGNAL_LENGTH));
        }
        if m > MAX_SIGNAL_LENGTH {
            return Err(NanoDtwError::SignalTooLong(m, MAX_SIGNAL_LENGTH));
        }

        let band_width = config.band_width.unwrap_or_else(|| {
            ((n as f32).max(m as f32) * 0.1) as usize
        }).max(10);

        let sakoe_band = config.sakoe_chiba_band.unwrap_or(band_width);

        if config.itakura_parallelogram {
            Self::itakura_dtw(signal, reference, config, band_width)
        } else {
            Self::sakoe_chiba_dtw(signal, reference, config, sakoe_band)
        }
    }

    fn sakoe_chiba_dtw(
        signal: &[f32],
        reference: &[f32],
        config: &DtwConfig,
        band_width: usize,
    ) -> Result<DtwAlignmentResult> {
        let n = signal.len();
        let m = reference.len();

        let mut prev_row = vec![f32::INFINITY; m + 1];
        let mut curr_row = vec![f32::INFINITY; m + 1];
        prev_row[0] = 0.0;

        let mut path_trace: Vec<Vec<u8>> = Vec::with_capacity(n);
        let mut row_bounds: Vec<(usize, usize)> = Vec::with_capacity(n);

        for i in 1..=n {
            let diag = (i as isize * m as isize / n as isize) as usize;
            let start_j = diag.saturating_sub(band_width).max(1);
            let end_j = (diag + band_width).min(m);

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

                let cost = distance(signal[i - 1], reference[j - 1], config.metric);
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

        Self::extract_banded_path(
            &path_trace,
            &row_bounds,
            prev_row[m],
            n,
            m,
            band_width,
        )
    }

    fn itakura_dtw(
        signal: &[f32],
        reference: &[f32],
        config: &DtwConfig,
        band_width: usize,
    ) -> Result<DtwAlignmentResult> {
        let n = signal.len();
        let m = reference.len();

        if n > MAX_SIGNAL_LENGTH || m > MAX_SIGNAL_LENGTH {
            return Self::sakoe_chiba_dtw(signal, reference, config, band_width);
        }

        let mut prev_row = vec![f32::INFINITY; m + 1];
        let mut curr_row = vec![f32::INFINITY; m + 1];
        prev_row[0] = 0.0;

        let mut path_trace: Vec<Vec<u8>> = Vec::with_capacity(n);
        let mut row_bounds: Vec<(usize, usize)> = Vec::with_capacity(n);

        let slope = m as f32 / n as f32;

        for i in 1..=n {
            let center_j = (i as f32 * slope) as usize;
            let start_j = center_j.saturating_sub(band_width).max(1);
            let end_j = (center_j + band_width).min(m);

            row_bounds.push((start_j, end_j));

            curr_row.fill(f32::INFINITY);

            let mut row_trace = vec![0u8; m + 1];

            for j in start_j..=end_j {
                if j > m {
                    break;
                }

                let j_ratio = j as f32 / i as f32;
                let i_ratio = i as f32 / j as f32;
                
                if j_ratio > 2.0 || i_ratio > 2.0 {
                    continue;
                }

                let cost = distance(signal[i - 1], reference[j - 1], config.metric);
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
            }

            path_trace.push(row_trace);
            std::mem::swap(&mut prev_row, &mut curr_row);
        }

        Self::extract_banded_path(
            &path_trace,
            &row_bounds,
            prev_row[m],
            n,
            m,
            band_width,
        )
    }

    fn extract_banded_path(
        path_trace: &[Vec<u8>],
        row_bounds: &[(usize, usize)],
        final_distance: f32,
        n: usize,
        m: usize,
        _band_width: usize,
    ) -> Result<DtwAlignmentResult> {
        let mut path = Vec::new();
        let (mut i, mut j) = (n, m);

        while i > 0 && j > 0 {
            path.push(DtwPathPoint {
                signal_idx: i - 1,
                reference_idx: j - 1,
                distance: 0.0,
            });

            let trace_idx = i - 1;
            if trace_idx >= path_trace.len() {
                log::warn!("Path trace index out of bounds: {} >= {}", trace_idx, path_trace.len());
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
                log::warn!("Column index out of bounds: {} >= {}", clamped_j, row_trace.len());
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

            if path.len() > n * 2 {
                log::warn!("Path too long, terminating early");
                break;
            }
        }

        path.reverse();

        let total_distance = final_distance;
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
            signal_end: n - 1,
            reference_start: 0,
            reference_end: m - 1,
        })
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dtw::core::DistanceMetric;

    #[test]
    fn test_safety_length_check() {
        let long_signal = vec![0.0; MAX_SIGNAL_LENGTH + 1];
        let reference = vec![1.0, 2.0, 3.0];
        let config = DtwConfig::default();
        
        let result = BandedDtw::align(&long_signal, &reference, &config);
        assert!(result.is_err());
    }

    #[test]
    fn test_bounded_path_extraction() {
        let signal: Vec<f32> = (0..100).map(|x| x as f32).collect();
        let reference: Vec<f32> = (0..100).map(|x| x as f32 * 1.1).collect();
        let config = DtwConfig::default().with_band_width(10);
        
        let result = BandedDtw::align(&signal, &reference, &config);
        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.path_length > 0);
        assert!(result.path_length <= (signal.len() + reference.len()) * 2);
        assert!(result.path_length >= signal.len().min(reference.len()));
    }

    #[test]
    fn test_noisy_sequence_alignment() {
        let mut signal = vec![0.0; 500];
        for i in 0..500 {
            signal[i] = if i % 3 == 0 { 100.0 } else { 0.0 };
        }
        let reference = vec![50.0; 200];
        let config = DtwConfig::default()
            .with_band_width(20)
            .with_metric(DistanceMetric::L1);
        
        let result = BandedDtw::align(&signal, &reference, &config);
        assert!(result.is_ok());
    }
}
