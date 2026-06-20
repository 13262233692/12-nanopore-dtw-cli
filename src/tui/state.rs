use parking_lot::Mutex;
use std::sync::Arc;
use std::time::{Duration, Instant};

const MAX_THROUGHPUT_SAMPLES: usize = 60;
const MAX_LOG_ENTRIES: usize = 200;

#[derive(Debug, Clone)]
pub struct LogEntry {
    pub level: LogLevel,
    pub message: String,
    pub timestamp: Instant,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PipelineStatus {
    Running,
    Paused,
    Completed,
    Failed,
}

pub struct TuiStateInner {
    pub total_reads: usize,
    pub processed_reads: usize,
    pub failed_reads: usize,
    pub total_bases: usize,
    pub queue_depth: usize,
    pub max_queue_depth: usize,
    pub status: PipelineStatus,
    pub throughput_history: Vec<f64>,
    pub logs: Vec<LogEntry>,
    pub start_time: Instant,
    pub last_processed: usize,
    pub last_throughput_update: Instant,
    pub file_count: usize,
    pub active_files: usize,
}

#[derive(Clone)]
pub struct TuiState {
    inner: Arc<Mutex<TuiStateInner>>,
}

impl TuiState {
    pub fn new(total_reads: usize, max_queue_depth: usize, file_count: usize) -> Self {
        let inner = TuiStateInner {
            total_reads,
            processed_reads: 0,
            failed_reads: 0,
            total_bases: 0,
            queue_depth: 0,
            max_queue_depth,
            status: PipelineStatus::Running,
            throughput_history: Vec::with_capacity(MAX_THROUGHPUT_SAMPLES),
            logs: Vec::with_capacity(MAX_LOG_ENTRIES),
            start_time: Instant::now(),
            last_processed: 0,
            last_throughput_update: Instant::now(),
            file_count,
            active_files: 0,
        };
        Self {
            inner: Arc::new(Mutex::new(inner)),
        }
    }

    pub fn update_processed(&self, processed: usize, bases: usize) {
        let mut state = self.inner.lock();
        state.processed_reads = processed;
        state.total_bases = bases;

        let now = Instant::now();
        let elapsed = now.duration_since(state.last_throughput_update);
        if elapsed >= Duration::from_millis(500) {
            let delta = processed.saturating_sub(state.last_processed) as f64;
            let throughput = delta / elapsed.as_secs_f64();
            state.throughput_history.push(throughput);
            if state.throughput_history.len() > MAX_THROUGHPUT_SAMPLES {
                state.throughput_history.remove(0);
            }
            state.last_processed = processed;
            state.last_throughput_update = now;
        }
    }

    pub fn increment_failed(&self) {
        let mut state = self.inner.lock();
        state.failed_reads += 1;
    }

    pub fn set_queue_depth(&self, depth: usize) {
        let mut state = self.inner.lock();
        state.queue_depth = depth.min(state.max_queue_depth);
    }

    pub fn set_active_files(&self, count: usize) {
        let mut state = self.inner.lock();
        state.active_files = count;
    }

    pub fn add_log(&self, level: LogLevel, message: String) {
        let mut state = self.inner.lock();
        let entry = LogEntry {
            level,
            message,
            timestamp: Instant::now(),
        };
        state.logs.push(entry);
        if state.logs.len() > MAX_LOG_ENTRIES {
            state.logs.remove(0);
        }
    }

    pub fn set_status(&self, status: PipelineStatus) {
        let mut state = self.inner.lock();
        state.status = status;
    }

    pub fn get_snapshot(&self) -> TuiSnapshot {
        let state = self.inner.lock();
        TuiSnapshot {
            total_reads: state.total_reads,
            processed_reads: state.processed_reads,
            failed_reads: state.failed_reads,
            total_bases: state.total_bases,
            queue_depth: state.queue_depth,
            max_queue_depth: state.max_queue_depth,
            status: state.status,
            throughput_history: state.throughput_history.clone(),
            logs: state.logs.clone(),
            elapsed: state.start_time.elapsed(),
            file_count: state.file_count,
            active_files: state.active_files,
        }
    }

    pub fn is_paused(&self) -> bool {
        let state = self.inner.lock();
        matches!(state.status, PipelineStatus::Paused)
    }

    pub fn toggle_pause(&self) -> bool {
        let mut state = self.inner.lock();
        match state.status {
            PipelineStatus::Running => {
                state.status = PipelineStatus::Paused;
                true
            }
            PipelineStatus::Paused => {
                state.status = PipelineStatus::Running;
                false
            }
            _ => false,
        }
    }
}

#[derive(Clone)]
pub struct TuiSnapshot {
    pub total_reads: usize,
    pub processed_reads: usize,
    pub failed_reads: usize,
    pub total_bases: usize,
    pub queue_depth: usize,
    pub max_queue_depth: usize,
    pub status: PipelineStatus,
    pub throughput_history: Vec<f64>,
    pub logs: Vec<LogEntry>,
    pub elapsed: Duration,
    pub file_count: usize,
    pub active_files: usize,
}

impl TuiSnapshot {
    pub fn reads_per_second(&self) -> f64 {
        if self.elapsed.as_secs_f64() > 0.0 {
            self.processed_reads as f64 / self.elapsed.as_secs_f64()
        } else {
            0.0
        }
    }

    pub fn progress_percent(&self) -> f64 {
        if self.total_reads > 0 {
            self.processed_reads as f64 / self.total_reads as f64 * 100.0
        } else {
            0.0
        }
    }

    pub fn queue_percent(&self) -> f64 {
        if self.max_queue_depth > 0 {
            self.queue_depth as f64 / self.max_queue_depth as f64 * 100.0
        } else {
            0.0
        }
    }
}
