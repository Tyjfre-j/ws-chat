mod handlers;
mod logging;
mod protocol;
mod state;

use axum::{Router, routing::get};
use dashmap::DashMap;
use std::sync::Arc;

use handlers::{handle_health, ws_handler};
use state::AppState;

#[tokio::main]
async fn main() {
    let _guard = logging::init();

    let state = AppState {
        rooms: Arc::new(DashMap::new()),
        usernames: Arc::new(DashMap::new()),
    };

    let app = Router::new()
        .route("/health", get(handle_health))
        .route("/ws", get(ws_handler))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    axum::serve(listener, app).await.unwrap();
}
