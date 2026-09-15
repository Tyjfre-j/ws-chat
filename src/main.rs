use axum::{
    Router,
    extract::State,
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
};

use dashmap::DashMap;
use futures_util::{SinkExt, StreamExt};
use std::sync::Arc;
use tokio::sync::broadcast;
#[derive(Clone)]
struct AppState {
    rooms: Arc<DashMap<String, broadcast::Sender<ServerMessage>>>,
}

#[derive(serde::Deserialize, Debug)]
#[serde(tag = "type", content = "data")]
enum ClientMessage {
    ChatMessage { message: String },
    JoinRoom { username: String, room: String },
}

#[derive(serde::Serialize, Debug, Clone)]
#[serde(tag = "type", content = "data")]
enum ServerMessage {
    ChatMessage { username: String, message: String },
    JoinedRoom { username: String },
    LeftRoom { username: String },
    Error { message: String },
}

async fn handle_health() -> &'static str {
    "OK"
}

async fn ws_handler(State(state): State<AppState>, ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    let Some(msg) = socket.recv().await else {
        return;
    };

    let Ok(msg) = msg else {
        return;
    };

    let Message::Text(text) = msg else {
        return;
    };

    let Ok(parsed) = serde_json::from_str::<ClientMessage>(&text) else {
        return;
    };

    let ClientMessage::JoinRoom { username, room } = parsed else {
        return;
    };

    let tx = state
        .rooms
        .entry(room.clone())
        .or_insert_with(|| broadcast::channel(16).0)
        .clone();

    let mut rx = tx.subscribe();

    let joined = ServerMessage::JoinedRoom {
        username: username.clone(),
    };
    let _ = tx.send(joined);

    let (mut sender, mut receiver) = socket.split();

    let send_task = tokio::spawn(async move {
        while let Ok(msg) = rx.recv().await {
            let text = serde_json::to_string(&msg).unwrap();
            if sender.send(Message::Text(text.into())).await.is_err() {
                break;
            }
        }
    });

    while let Some(msg) = receiver.next().await {
        if let Ok(msg) = msg {
            match msg {
                Message::Text(text) => {
                    if let Ok(parsed) = serde_json::from_str::<ClientMessage>(&text) {
                        println!("Received valid message: {:?}", parsed);

                        match parsed {
                            ClientMessage::ChatMessage { message } => {
                                let reply = ServerMessage::ChatMessage {
                                    username: username.clone(),
                                    message,
                                };
                                let _ = tx.send(reply);
                            }
                            ClientMessage::JoinRoom { .. } => {
                                let reply = ServerMessage::Error {
                                    message: "already joined".to_string(),
                                };
                                let _ = tx.send(reply);
                            }
                        }
                    } else {
                        println!("Failed to parse message: {}", text);

                        let error_reply = ServerMessage::Error {
                            message: "invalid message format".into(),
                        };
                        let _ = tx.send(error_reply);
                    }
                }
                _ => {}
            }
        } else {
            break;
        }
    }

    let left = ServerMessage::LeftRoom {
        username: username.clone(),
    };
    let _ = tx.send(left);

    send_task.abort();
}

#[tokio::main]
async fn main() {
    let state = AppState {
        rooms: Arc::new(DashMap::new()),
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
