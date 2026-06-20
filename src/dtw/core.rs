use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum DistanceMetric {
    L1,
    L2,
    Lp(f32),
}

impl Default for DistanceMetric {
    fn default() -> Self {
        DistanceMetric::L2
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct DtwPathPoint {
    pub signal_idx: usize,
    pub reference_idx: usize,
    pub distance: f32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DtwAlignmentResult {
    pub total_distance: f32,
    pub normalized_distance: f32,
    pub path_length: usize,
    pub alignment_path: Vec<crate::types::DtwPathPoint>,
    pub signal_start: usize,
    pub signal_end: usize,
    pub reference_start: usize,
    pub reference_end: usize,
}

#[derive(Clone)]
pub struct DtwConfig {
    pub metric: DistanceMetric,
    pub band_width: Option<usize>,
    pub max_distance: Option<f32>,
    pub window_fn: fn(f32, usize, usize) -> f32,
    pub step_pattern: StepPattern,
    pub normalize: bool,
    pub sakoe_chiba_band: Option<usize>,
    pub itakura_parallelogram: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepPattern {
    Symmetric1,
    Symmetric2,
    Asymmetric,
    Diagonal,
}

impl Default for DtwConfig {
    fn default() -> Self {
        Self {
            metric: DistanceMetric::default(),
            band_width: Some(100),
            max_distance: Some(1e6),
            window_fn: |cost, _, _| cost,
            step_pattern: StepPattern::Symmetric2,
            normalize: true,
            sakoe_chiba_band: Some(50),
            itakura_parallelogram: false,
        }
    }
}

impl DtwConfig {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_metric(mut self, metric: DistanceMetric) -> Self {
        self.metric = metric;
        self
    }

    pub fn with_band_width(mut self, bw: usize) -> Self {
        self.band_width = Some(bw);
        self.sakoe_chiba_band = Some(bw);
        self
    }

    pub fn with_max_distance(mut self, max: f32) -> Self {
        self.max_distance = Some(max);
        self
    }

    pub fn without_band(mut self) -> Self {
        self.band_width = None;
        self.sakoe_chiba_band = None;
        self
    }

    pub fn with_step_pattern(mut self, pattern: StepPattern) -> Self {
        self.step_pattern = pattern;
        self
    }
}

#[inline]
pub fn distance(a: f32, b: f32, metric: DistanceMetric) -> f32 {
    match metric {
        DistanceMetric::L1 => (a - b).abs(),
        DistanceMetric::L2 => (a - b).powi(2),
        DistanceMetric::Lp(p) => (a - b).abs().powf(p),
    }
}

#[inline]
pub fn distance_squared(a: f32, b: f32) -> f32 {
    let diff = a - b;
    diff * diff
}

#[inline]
pub fn distance_abs(a: f32, b: f32) -> f32 {
    (a - b).abs()
}

pub struct CostMatrix {
    data: Vec<f32>,
    _rows: usize,
    cols: usize,
}

impl CostMatrix {
    pub fn new(rows: usize, cols: usize) -> Self {
        Self {
            data: vec![f32::INFINITY; rows * cols],
            _rows: rows,
            cols,
        }
    }

    #[inline]
    pub fn get(&self, i: usize, j: usize) -> f32 {
        self.data[i * self.cols + j]
    }

    #[inline]
    pub fn set(&mut self, i: usize, j: usize, val: f32) {
        self.data[i * self.cols + j] = val;
    }

    #[inline]
    pub fn get_mut(&mut self, i: usize, j: usize) -> &mut f32 {
        &mut self.data[i * self.cols + j]
    }
}
