pub mod collector;
pub mod config;
pub mod dashboard;
pub mod models;
pub mod state;
pub mod system;
pub mod web;

use std::{net::SocketAddr, sync::Arc};

use collector::Collector;
use config::Config;
use state::{AppState, SharedState};
use tokio::net::TcpListener;

/// Start the monitoring server and collector loop.
///
/// # Errors
///
/// Returns an error if binding the listener or serving HTTP fails.
pub async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env_and_args();
    let hostname = system::read_hostname().unwrap_or_else(|| "raspberrypi".to_string());

    let shared = Arc::new(SharedState::new(hostname, config.history_size));

    let collector_state = Arc::clone(&shared);
    let collector_config = config.clone();
    let collector_task = tokio::spawn(async move {
        let mut collector = Collector::new();
        collector.run(collector_state, collector_config).await;
    });

    let app = web::router(AppState {
        shared: Arc::clone(&shared),
    });
    let addr = SocketAddr::new(config.host, config.port);
    let listener = TcpListener::bind(addr).await?;

    println!("listening on http://{addr}");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    collector_task.abort();
    Ok(())
}

async fn shutdown_signal() {
    let _ = tokio::signal::ctrl_c().await;
}
