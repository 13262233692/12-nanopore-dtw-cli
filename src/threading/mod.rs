pub mod pipeline;
pub mod worker;

pub use pipeline::ProcessingPipeline;
pub use worker::WorkerPool;


use crate::types::{DtwResult, RawSignal};

pub enum PipelineMessage {
    Signal(RawSignal),
    Batch(Vec<RawSignal>),
    Result(DtwResult),
    Terminate,
    Error(crate::error::NanoDtwError),
    Stats(ProcessStats),
}

use crate::types::ProcessStats;
