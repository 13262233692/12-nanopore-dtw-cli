use crate::dtw::core::{distance, DtwConfig, DtwAlignmentResult, DtwPathPoint};
use crate::error::{NanoDtwError, Result};
use crate::types::DtwPathPoint as TypesDtwPathPoint;

pub struct BandedDtw;

impl BandedDtw {
    pub fn align(
        signal: &[f32],
        reference: &[f32],
        config: &DtwConfig,
    ) -> Result<DtwAlignmentResult> {
        let n = signal.len();
        let m = reference.len();

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

        for i in 1..=n {
            let diag = (i as isize * m as isize / n as isize) as usize;
            let start_j = diag.saturating_sub(band_width).max(1);
            let end_j = (diag + band_width).min(m);

            curr_row.fill(f32::INFINITY);
            if i == 1 {
                curr_row[0] = f32::INFINITY;
            }

            let mut row_trace = vec![0u8; m + 1];

            for j in start_j..=end_j {
                let cost = distance(signal[i - 1], reference[j - 1], config.metric);
                let cost = (config.window_fn)(cost, i - 1, j - 1);

                let diag_val = prev_row[j - 1];
                let up_val = prev_row[j];
                let left_val = curr_row[j - 1];

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

        let mut dtw = vec![vec![f32::INFINITY; m + 1]; n + 1];
        dtw[0][0] = 0.0;

        let slope = m as f32 / n as f32;

        for i in 1..=n {
            let center_j = (i as f32 * slope) as usize;
            let start_j = center_j.saturating_sub(band_width).max(1);
            let end_j = (center_j + band_width).min(m);

            for j in start_j..=end_j {
                let j_ratio = j as f32 / i as f32;
                let i_ratio = i as f32 / j as f32;
                
                if j_ratio > 2.0 || i_ratio > 2.0 {
                    continue;
                }

                let cost = distance(signal[i - 1], reference[j - 1], config.metric);
                let cost = (config.window_fn)(cost, i - 1, j - 1);

                dtw[i][j] = cost + dtw[i - 1][j - 1].min(dtw[i - 1][j]).min(dtw[i][j - 1]);
            }
        }

        Self::extract_path(&dtw, n, m, n, m)
    }

    fn extract_banded_path(
        path_trace: &[Vec<u8>],
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

            let direction = path_trace[i - 1][j];
            match direction {
                0b01 => {
                    i -= 1;
                    j -= 1;
                }
                0b10 => {
                    i -= 1;
                }
                0b11 => {
                    j -= 1;
                }
                _ => {
                    let _min_j = (j as isize - 1).max(1) as usize;
                    let _max_j = (j + 1).min(m);
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
