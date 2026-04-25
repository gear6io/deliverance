use async_trait::async_trait;
use tokio::sync::mpsc;

use crate::error::{IngestionError, Result};
use crate::exporter::Exporter;
use crate::processor::Processor;
use crate::receiver::{Receiver, ReceiverHost};
use crate::types::Event;

/// Default channel capacity between Receiver and the processor chain.
/// When the channel is full, `ChannelReceiverHost::emit()` awaits — this IS backpressure (F2).
pub const DEFAULT_CHANNEL_CAPACITY: usize = 1024;

/// The framework-owned concrete ReceiverHost. Wraps the sender half of the pipeline channel.
pub struct ChannelReceiverHost {
    tx: mpsc::Sender<Event>,
}

impl ChannelReceiverHost {
    pub fn new(tx: mpsc::Sender<Event>) -> Self {
        Self { tx }
    }
}

#[async_trait]
impl ReceiverHost for ChannelReceiverHost {
    async fn emit(&self, event: Event) -> Result<()> {
        self.tx.send(event).await.map_err(|_| {
            IngestionError::ChannelSend("pipeline channel closed".into())
        })
    }
}

/// A group of exporters as configured in the pipeline's exporters list.
/// Built by the ComponentRegistry from ExporterEntry config.
pub enum ExporterGroup {
    /// Single mandatory exporter. Failure triggers retry + drop after max attempts.
    Flat(Box<dyn Exporter>),
    /// Try each exporter in order; first success wins. If all fail, retry the group.
    OneOf(Vec<Box<dyn Exporter>>),
}

/// Fully constructed pipeline ready for the Engine to drive.
pub struct Pipeline {
    pub name: String,
    pub receiver: Box<dyn Receiver>,
    pub processors: Vec<Box<dyn Processor>>,
    pub exporter_groups: Vec<ExporterGroup>,
    pub channel_capacity: usize,
}
