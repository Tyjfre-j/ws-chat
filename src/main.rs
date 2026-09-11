use axum::{
    Router,
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
};

async fn handle_health() -> &'static str {
    "OK"
}

async fn ws_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_socket)
}

async fn handle_socket(mut socket: WebSocket) {
    while let Some(msg) = socket.recv().await {
        if let Ok(msg) = msg {
            match msg {
                Message::Text(_) | Message::Binary(_) => {
                    if socket.send(msg).await.is_err() {
                        return;
                    }
                }
                _ => {}
            }
        } else {
            return;
        }
    }
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/health", get(handle_health))
        .route("/ws", get(ws_handler));

    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000")
        .await
        .unwrap();

    axum::serve(listener, app).await.unwrap();
}
