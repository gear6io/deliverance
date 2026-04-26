use std::env;

use ingestion::{ComponentRegistry, Config, Engine};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let config_path = env::args()
        .nth(1)
        .unwrap_or_else(|| "config.yaml".to_string());

    let config = Config::from_file(&config_path)?;

    let registry = build_registry();

    let pipelines = registry.build_all(&config)?;
    let mut engine = Engine::new(pipelines);

    let handles = engine.start().await?;
    tracing::info!("engine started — waiting for Ctrl+C");

    tokio::signal::ctrl_c().await?;
    tracing::info!("shutdown signal received");

    engine.shutdown().await?;

    for handle in handles {
        let _ = handle.await;
    }

    tracing::info!("engine stopped");
    Ok(())
}

fn build_registry() -> ComponentRegistry {
    use ingestion::exporter::debug::DebugExporterFactory;
    use ingestion::processor::batch::BatchProcessorFactory;
    use ingestion::processor::memory::MemoryProcessorFactory;
    use ingestion::receiver::http::HttpReceiverFactory;

    let mut registry = ComponentRegistry::new();

    registry.register_receiver(Box::new(HttpReceiverFactory));
    registry.register_processor(Box::new(MemoryProcessorFactory));
    registry.register_processor(Box::new(BatchProcessorFactory));
    registry.register_exporter(Box::new(DebugExporterFactory));
    registry
}
