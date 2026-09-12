use axum::{
    Router,
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
};

#[derive(serde::Deserialize, Debug)]
#[serde(tag = "type", content = "data")]
enum ClientMessage {
    ChatMessage { message: String },
}

#[derive(serde::Serialize, Debug)]
#[serde(tag = "type", content = "data")]
enum ServerMessage {
    ChatMessage { message: String },
    Error { message: String },
}

async fn handle_health() -> &'static str {
    "OK"
}

async fn send_server_message(socket: &mut WebSocket, msg: &ServerMessage) -> bool {
    let text = serde_json::to_string(msg).unwrap();
    socket.send(Message::Text(text.into())).await.is_ok()
}

async fn ws_handler(ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(handle_socket)
}

async fn handle_socket(mut socket: WebSocket) {
    while let Some(msg) = socket.recv().await {
        if let Ok(msg) = msg {
            match msg {
                Message::Text(text) => {
                    if let Ok(parsed) = serde_json::from_str::<ClientMessage>(&text) {
                        println!("Received valid message: {:?}", parsed);

                        match parsed {
                            ClientMessage::ChatMessage { message } => {
                                let reply: ServerMessage = ServerMessage::ChatMessage { message };
                                if !send_server_message(&mut socket, &reply).await {
                                    return;
                                }
                            }
                        }
                    } else {
                        println!("Failed to parse message: {}", text);

                        let error_reply: ServerMessage = ServerMessage::Error {
                            message: "invalid message format".into(),
                        };

                        if !send_server_message(&mut socket, &error_reply).await {
                            return;
                        }
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
