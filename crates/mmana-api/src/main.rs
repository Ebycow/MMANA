//! `mmana-api` server.
//!
//! Environment:
//! - `MMANA_ADDR` (default `127.0.0.1:8080`)
//! - `MMANA_MAX_PULSES` (default 4096)
//! - `MMANA_MAX_OPTIMIZE_CALCULATIONS` (default 2000)
//! - `RUST_LOG` (default `info`)

use mmana_api::{AppState, Limits, app};

#[tokio::main]
async fn main() -> std::io::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .init();

    let addr = std::env::var("MMANA_ADDR").unwrap_or_else(|_| "127.0.0.1:8080".into());
    let mut limits = Limits::default();
    if let Some(n) = std::env::var("MMANA_MAX_PULSES").ok().and_then(|v| v.parse().ok()) {
        limits.max_pulses = n;
    }
    if let Some(n) = std::env::var("MMANA_MAX_OPTIMIZE_CALCULATIONS").ok().and_then(|v| v.parse().ok()) {
        limits.max_optimize_calculations = n;
    }

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    tracing::info!("listening on http://{}", listener.local_addr()?);
    axum::serve(listener, app(AppState { limits }))
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await
}
