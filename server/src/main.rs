mod handlers;
mod logging;
mod protocol;
mod state;

use axum::{Router, routing::get};
use dashmap::DashMap;
use std::future::IntoFuture;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::oneshot;

use handlers::{handle_health, ws_handler};
use state::AppState;

const DEFAULT_SERVER_ADDR: &str = "127.0.0.1:3000";

#[tokio::main]
async fn main() -> std::io::Result<()> {
    let _guard = logging::init();

    let state = AppState {
        rooms: Arc::new(DashMap::new()),
        usernames: Arc::new(DashMap::new()),
    };

    let app = Router::new()
        .route("/health", get(handle_health))
        .route("/ws", get(ws_handler))
        .with_state(state);

    let server_addr =
        std::env::var("WS_CHAT_SERVER_ADDR").unwrap_or_else(|_| DEFAULT_SERVER_ADDR.to_string());
    let listener = tokio::net::TcpListener::bind(&server_addr).await?;

    let (shutdown_tx, shutdown_rx) = oneshot::channel();
    let server = axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            let _ = shutdown_rx.await;
        })
        .into_future();
    tokio::pin!(server);

    tokio::select! {
        result = &mut server => result,
        _ = shutdown_signal() => {
            let _ = shutdown_tx.send(());
            match tokio::time::timeout(Duration::from_secs(10), &mut server).await {
                Ok(result) => result,
                Err(_) => {
                    tracing::warn!("graceful shutdown timed out; closing active connections");
                    Ok(())
                }
            }
        }
    }
}

async fn shutdown_signal() {
    if let Err(error) = tokio::signal::ctrl_c().await {
        tracing::warn!(%error, "failed to listen for shutdown signal");
    }
}
