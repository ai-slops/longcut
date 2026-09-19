use std::{env, path::PathBuf};

use anyhow::Context;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .json()
        .with_target(false)
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,tower_http=info,sqlx=warn")),
        )
        .init();
    let bind = env::var("LONGCUT_BIND").unwrap_or_else(|_| "0.0.0.0:4000".into());
    let data = PathBuf::from(env::var("LONGCUT_DATA").unwrap_or_else(|_| "/data".into()));
    let state = longcut_server::AppState::initialize(&data)
        .await
        .context("initialize longcut data")?;
    let listener = tokio::net::TcpListener::bind(&bind)
        .await
        .with_context(|| format!("bind {bind}"))?;
    tracing::info!(address = %bind, data = %data.display(), "server.listening");
    axum::serve(listener, longcut_server::app(state))
        .with_graceful_shutdown(shutdown())
        .await?;
    Ok(())
}

async fn shutdown() {
    #[cfg(unix)]
    {
        let terminate = async {
            match tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()) {
                Ok(mut signal) => {
                    signal.recv().await;
                }
                Err(error) => tracing::warn!(error = ?error, "signal.install_failed"),
            }
        };
        tokio::select! { result = tokio::signal::ctrl_c() => { if let Err(error) = result { tracing::warn!(error = ?error, "signal.ctrl_c_failed"); } }, _ = terminate => {} }
    }
    #[cfg(not(unix))]
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::warn!(error = ?error, "signal.ctrl_c_failed");
    }
    tracing::info!("server.shutdown");
}
