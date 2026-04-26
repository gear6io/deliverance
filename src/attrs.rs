/// Reserved keys for `Event::source`.
///
/// **Set by:** receivers, when constructing an [`Event`] before calling [`ReceiverHost::emit`].
/// **Read by:** processors and exporters that need to inspect or route on source identity.
///
/// Keys follow OTel semantic-convention style: `<namespace>.<attribute>`.
///
/// [`Event`]: crate::types::Event
/// [`ReceiverHost::emit`]: crate::receiver::ReceiverHost::emit

pub mod datasource {
    /// Unique identifier of the datasource this event originated from.
    ///
    /// Set by: every receiver — e.g. `receiver/http` maps `IngestRequest::datasource_id` here.
    /// Read by: exporters and processors that route or partition per datasource.
    pub const ID: &str = "datasource.id";

    /// Human-readable name of the datasource (optional).
    ///
    /// Set by: receivers that have access to a display name alongside the ID.
    /// Read by: exporters that surface datasource identity in logs or metrics labels.
    pub const NAME: &str = "datasource.name";

    /// Transport/protocol kind of the receiver that produced this event.
    ///
    /// Set by: receivers to identify their protocol (e.g. `"http"`, `"grpc"`, `"kafka"`).
    /// Read by: processors or exporters that need to vary behaviour per ingestion path.
    pub const KIND: &str = "datasource.kind";
}
