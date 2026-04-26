use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;

use arrow::record_batch::RecordBatch;

/// Internal scalar type system — maps 1:1 to the proto Value kinds.
/// Receivers normalize inbound data to these types when building Arrow records.
#[derive(Debug, Clone)]
pub enum Value {
    String(String),
    Bytes(Vec<u8>),
    Int(i64),
    Float(f64),
    Bool(bool),
    Array(Vec<Value>),
    Map(HashMap<String, Value>),
}

/// The unit a Receiver emits into the pipeline.
///
/// `record` is `Arc<RecordBatch>` — cloning is a refcount bump, not a buffer copy.
/// This is the zero-copy guarantee: Arrow buffers never duplicate as an Event flows
/// from Receiver → channel → Processors → Executor → Exporter.
#[derive(Debug, Clone)]
pub struct Event {
    pub source: HashMap<String, Value>,
    pub record: Arc<RecordBatch>,
    pub received_at: SystemTime,
}

/// The unit flushed by the BatchingProcessor and handed to every Exporter.
/// Events are not merged per Source — a Batch may contain multiple entries for the
/// same Source from separate Receiver emits (mirrors OTel ResourceLogs model).
#[derive(Debug, Clone)]
pub struct Batch {
    pub events: Vec<Event>,
}

impl Batch {
    pub fn new() -> Self {
        Self { events: Vec::new() }
    }

    pub fn push(&mut self, event: Event) {
        self.events.push(event);
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Total row count across all events (sum of RecordBatch num_rows).
    pub fn row_count(&self) -> usize {
        self.events.iter().map(|e| e.record.num_rows()).sum()
    }
}

impl Default for Batch {
    fn default() -> Self {
        Self::new()
    }
}
