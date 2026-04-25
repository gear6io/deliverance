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

    // Build the registry with built-in processors registered.
    // Add custom receivers and exporters here before calling build_all().
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
    use ingestion::processor::{batch::BatchingProcessor, memory::MemoryProcessor};
    use ingestion::receiver::http::HttpReceiver;

    let mut registry = ComponentRegistry::new();

    registry.register_receiver("http", Box::new(HttpReceiver::from_config));
    registry.register_processor("memory", Box::new(MemoryProcessor::from_config));
    registry.register_processor("batch", Box::new(BatchingProcessor::from_config));

    registry
}
