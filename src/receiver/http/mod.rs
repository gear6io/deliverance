pub mod config;

use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;

use async_trait::async_trait;
use axum::{body::Bytes, extract::State, http::StatusCode, response::Json, routing::post, Router};
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use arrow::array::{
    Array, ArrayRef, BooleanBuilder, Float64Builder, Int64Builder, LargeStringBuilder, ListBuilder,
    MapBuilder, StringBuilder,
};
use arrow::datatypes::{DataType, Field, Schema};
use arrow::record_batch::RecordBatch;

use crate::components::Component;
use crate::error::{IngestionError, Result};
use crate::receiver::{Receiver, ReceiverHost};
use crate::types::{Event, Source};

pub use config::HttpReceiverConfig;

pub struct HttpReceiver {
    config: HttpReceiverConfig,
    shutdown_tx: Option<oneshot::Sender<()>>,
    server_handle: Option<JoinHandle<()>>,
}

impl Component for HttpReceiver {
    fn name() -> &'static str {
        "httpreceiver"
    }

    fn from_yaml(raw: &serde_yaml::Value) -> anyhow::Result<Self> {
        let cfg: HttpReceiverConfig = serde_yaml::from_value(raw.clone())?;
        Ok(HttpReceiver {
            config: cfg,
            shutdown_tx: None,
            server_handle: None,
        })
    }
}

impl HttpReceiver {
    pub fn from_config(raw: &serde_yaml::Value) -> anyhow::Result<Box<dyn Receiver>> {
        Ok(Box::new(Self::from_yaml(raw)?))
    }
}

#[async_trait]
impl Receiver for HttpReceiver {
    async fn start(&mut self, host: Arc<dyn ReceiverHost>) -> Result<()> {
        let (shutdown_tx, shutdown_rx) = oneshot::channel::<()>();

        let app = Router::new()
            .route("/v1/events", post(handle_ingest))
            .with_state(host);

        let listener = TcpListener::bind(&self.config.endpoint)
            .await
            .map_err(IngestionError::Io)?;

        tracing::info!(endpoint = %self.config.endpoint, "HTTP receiver listening");

        let handle = tokio::spawn(async move {
            let shutdown_signal = async move {
                let _ = shutdown_rx.await;
            };
            if let Err(e) = axum::serve(listener, app)
                .with_graceful_shutdown(shutdown_signal)
                .await
            {
                tracing::error!("HTTP server error: {e}");
            }
        });

        self.shutdown_tx = Some(shutdown_tx);
        self.server_handle = Some(handle);
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
        if let Some(handle) = self.server_handle.take() {
            let _ = handle.await;
        }
        Ok(())
    }
}

// ── Handler ───────────────────────────────────────────────────────────────────

async fn handle_ingest(
    State(host): State<Arc<dyn ReceiverHost>>,
    body: Bytes,
) -> (StatusCode, Json<serde_json::Value>) {
    let payload: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(v) => v,
        Err(e) => return err(StatusCode::BAD_REQUEST, &e.to_string()),
    };

    let datasource_id = match payload.get("datasource_id").and_then(|v| v.as_str()) {
        Some(s) => s.to_string(),
        None => return err(StatusCode::BAD_REQUEST, "missing datasource_id"),
    };

    let raw_rows = match payload.get("rows").and_then(|v| v.as_array()) {
        Some(r) if !r.is_empty() => r,
        Some(_) => return err(StatusCode::BAD_REQUEST, "rows must be non-empty"),
        None => return err(StatusCode::BAD_REQUEST, "missing or invalid rows"),
    };

    let row_maps: Vec<&serde_json::Map<String, serde_json::Value>> =
        match raw_rows.iter().map(|v| v.as_object().ok_or(())).collect() {
            Ok(maps) => maps,
            Err(_) => return err(StatusCode::BAD_REQUEST, "each row must be a JSON object"),
        };

    let record_batch = match build_record_batch(&row_maps) {
        Ok(rb) => rb,
        Err(e) => return err(StatusCode::BAD_REQUEST, &e.to_string()),
    };

    let event = Event {
        source: Source {
            id: datasource_id,
            attributes: HashMap::new(),
        },
        record: Arc::new(record_batch),
        received_at: SystemTime::now(),
    };

    match host.emit(event).await {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"accepted": true}))),
        Err(e) => err(StatusCode::INTERNAL_SERVER_ERROR, &e.to_string()),
    }
}

fn err(code: StatusCode, msg: &str) -> (StatusCode, Json<serde_json::Value>) {
    (code, Json(serde_json::json!({"error": msg})))
}

// ── Arrow RecordBatch construction ────────────────────────────────────────────

fn build_record_batch(
    rows: &[&serde_json::Map<String, serde_json::Value>],
) -> anyhow::Result<RecordBatch> {
    let first = rows[0];
    let mut fields: Vec<Field> = Vec::new();
    let mut columns: Vec<ArrayRef> = Vec::new();

    for (name, first_val) in first {
        let (field, col) = build_column(name, first_val, rows)?;
        fields.push(field);
        columns.push(col);
    }

    let schema = Arc::new(Schema::new(fields));
    Ok(RecordBatch::try_new(schema, columns)?)
}

fn build_column(
    name: &str,
    first_val: &serde_json::Value,
    rows: &[&serde_json::Map<String, serde_json::Value>],
) -> anyhow::Result<(Field, ArrayRef)> {
    match first_val {
        serde_json::Value::String(_) => {
            let mut b = StringBuilder::new();
            for row in rows {
                match row.get(name) {
                    None | Some(serde_json::Value::Null) => b.append_null(),
                    Some(serde_json::Value::String(s)) => b.append_value(s),
                    Some(v) => {
                        anyhow::bail!("field '{name}': expected string, got {}", json_type(v))
                    }
                }
            }
            Ok((Field::new(name, DataType::Utf8, true), Arc::new(b.finish())))
        }

        serde_json::Value::Number(n) if n.is_i64() => {
            let mut b = Int64Builder::new();
            for row in rows {
                match row.get(name) {
                    None | Some(serde_json::Value::Null) => b.append_null(),
                    Some(serde_json::Value::Number(n)) => match n.as_i64() {
                        Some(i) => b.append_value(i),
                        None => anyhow::bail!("field '{name}': expected integer, got float"),
                    },
                    Some(v) => {
                        anyhow::bail!("field '{name}': expected integer, got {}", json_type(v))
                    }
                }
            }
            Ok((
                Field::new(name, DataType::Int64, true),
                Arc::new(b.finish()),
            ))
        }

        serde_json::Value::Number(_) => {
            let mut b = Float64Builder::new();
            for row in rows {
                match row.get(name) {
                    None | Some(serde_json::Value::Null) => b.append_null(),
                    Some(serde_json::Value::Number(n)) => match n.as_f64() {
                        Some(f) => b.append_value(f),
                        None => anyhow::bail!("field '{name}': number out of f64 range"),
                    },
                    Some(v) => {
                        anyhow::bail!("field '{name}': expected number, got {}", json_type(v))
                    }
                }
            }
            Ok((
                Field::new(name, DataType::Float64, true),
                Arc::new(b.finish()),
            ))
        }

        serde_json::Value::Bool(_) => {
            let mut b = BooleanBuilder::new();
            for row in rows {
                match row.get(name) {
                    None | Some(serde_json::Value::Null) => b.append_null(),
                    Some(serde_json::Value::Bool(v)) => b.append_value(*v),
                    Some(v) => {
                        anyhow::bail!("field '{name}': expected boolean, got {}", json_type(v))
                    }
                }
            }
            Ok((
                Field::new(name, DataType::Boolean, true),
                Arc::new(b.finish()),
            ))
        }

        serde_json::Value::Array(arr) => {
            if arr.is_empty() {
                anyhow::bail!(
                    "field '{name}': cannot infer element type from empty array in first row"
                );
            }
            build_list_column(name, &arr[0], rows)
        }

        serde_json::Value::Object(_) => build_map_column(name, rows),

        serde_json::Value::Null => {
            anyhow::bail!("field '{name}': first row must not be null — cannot infer column type")
        }
    }
}

fn build_list_column(
    name: &str,
    first_elem: &serde_json::Value,
    rows: &[&serde_json::Map<String, serde_json::Value>],
) -> anyhow::Result<(Field, ArrayRef)> {
    match first_elem {
        serde_json::Value::String(_) => {
            let mut b = ListBuilder::new(StringBuilder::new());
            for row in rows {
                match row.get(name) {
                    None | Some(serde_json::Value::Null) => b.append_null(),
                    Some(serde_json::Value::Array(arr)) => {
                        for v in arr {
                            match v {
                                serde_json::Value::String(s) => b.values().append_value(s),
                                serde_json::Value::Null => b.values().append_null(),
                                _ => anyhow::bail!(
                                    "field '{name}': array element type mismatch, expected string"
                                ),
                            }
                        }
                        b.append(true);
                    }
                    Some(v) => {
                        anyhow::bail!("field '{name}': expected array, got {}", json_type(v))
                    }
                }
            }
            let arr = b.finish();
            let dt = arr.data_type().clone();
            Ok((Field::new(name, dt, true), Arc::new(arr)))
        }

        serde_json::Value::Number(n) if n.is_i64() => {
            let mut b = ListBuilder::new(Int64Builder::new());
            for row in rows {
                match row.get(name) {
                    None | Some(serde_json::Value::Null) => b.append_null(),
                    Some(serde_json::Value::Array(arr)) => {
                        for v in arr {
                            if v.is_null() {
                                b.values().append_null();
                            } else {
                                match v.as_i64() {
                                    Some(i) => b.values().append_value(i),
                                    None => anyhow::bail!(
                                        "field '{name}': array element type mismatch, expected integer"
                                    ),
                                }
                            }
                        }
                        b.append(true);
                    }
                    Some(v) => {
                        anyhow::bail!("field '{name}': expected array, got {}", json_type(v))
                    }
                }
            }
            let arr = b.finish();
            let dt = arr.data_type().clone();
            Ok((Field::new(name, dt, true), Arc::new(arr)))
        }

        serde_json::Value::Number(_) => {
            let mut b = ListBuilder::new(Float64Builder::new());
            for row in rows {
                match row.get(name) {
                    None | Some(serde_json::Value::Null) => b.append_null(),
                    Some(serde_json::Value::Array(arr)) => {
                        for v in arr {
                            if v.is_null() {
                                b.values().append_null();
                            } else {
                                match v.as_f64() {
                                    Some(f) => b.values().append_value(f),
                                    None => anyhow::bail!(
                                        "field '{name}': array element type mismatch, expected number"
                                    ),
                                }
                            }
                        }
                        b.append(true);
                    }
                    Some(v) => {
                        anyhow::bail!("field '{name}': expected array, got {}", json_type(v))
                    }
                }
            }
            let arr = b.finish();
            let dt = arr.data_type().clone();
            Ok((Field::new(name, dt, true), Arc::new(arr)))
        }

        serde_json::Value::Bool(_) => {
            let mut b = ListBuilder::new(BooleanBuilder::new());
            for row in rows {
                match row.get(name) {
                    None | Some(serde_json::Value::Null) => b.append_null(),
                    Some(serde_json::Value::Array(arr)) => {
                        for v in arr {
                            if v.is_null() {
                                b.values().append_null();
                            } else {
                                match v.as_bool() {
                                    Some(bv) => b.values().append_value(bv),
                                    None => anyhow::bail!(
                                        "field '{name}': array element type mismatch, expected boolean"
                                    ),
                                }
                            }
                        }
                        b.append(true);
                    }
                    Some(v) => {
                        anyhow::bail!("field '{name}': expected array, got {}", json_type(v))
                    }
                }
            }
            let arr = b.finish();
            let dt = arr.data_type().clone();
            Ok((Field::new(name, dt, true), Arc::new(arr)))
        }

        v => anyhow::bail!(
            "field '{name}': unsupported array element type '{}'",
            json_type(v)
        ),
    }
}

fn build_map_column(
    name: &str,
    rows: &[&serde_json::Map<String, serde_json::Value>],
) -> anyhow::Result<(Field, ArrayRef)> {
    let mut b = MapBuilder::new(None, StringBuilder::new(), LargeStringBuilder::new());
    for row in rows {
        match row.get(name) {
            None | Some(serde_json::Value::Null) => {
                b.append(false)?;
            }
            Some(serde_json::Value::Object(obj)) => {
                for (k, v) in obj {
                    b.keys().append_value(k);
                    b.values().append_value(serde_json::to_string(v)?);
                }
                b.append(true)?;
            }
            Some(v) => anyhow::bail!("field '{name}': expected object, got {}", json_type(v)),
        }
    }
    let arr = b.finish();
    let dt = arr.data_type().clone();
    Ok((Field::new(name, dt, true), Arc::new(arr)))
}

fn json_type(v: &serde_json::Value) -> &'static str {
    match v {
        serde_json::Value::Null => "null",
        serde_json::Value::Bool(_) => "boolean",
        serde_json::Value::Number(_) => "number",
        serde_json::Value::String(_) => "string",
        serde_json::Value::Array(_) => "array",
        serde_json::Value::Object(_) => "object",
    }
}
