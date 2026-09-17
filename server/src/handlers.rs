use axum::{
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    extract::State,
    response::IntoResponse,
};
use futures_util::{SinkExt, StreamExt};
use tokio::sync::broadcast;

use crate::protocol::{ClientMessage, Received, ServerMessage};
use crate::state::AppState;

pub async fn handle_health() -> &'static str {
    "OK"
}

pub async fn ws_handler(State(state): State<AppState>, ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn send_error(socket: &mut WebSocket, message: &str) {
    let error_reply = ServerMessage::Error {
        message: message.to_string(),
    };
    let error_text = serde_json::to_string(&error_reply).unwrap();
    let _ = socket.send(Message::Text(error_text.into())).await;
}

async fn receive_client_message(socket: &mut WebSocket) -> Received {
    let Some(msg) = socket.recv().await else {
        return Received::Disconnected; 
    };
    let Ok(msg) = msg else {
        return Received::Disconnected; 
    };
    let Message::Text(text) = msg else {
        return Received::Invalid; 
    };
    match serde_json::from_str::<ClientMessage>(&text) {
        Ok(parsed) => Received::Message(parsed),
        Err(_) => Received::Invalid,
    }
}

async fn welcome_user(socket: &mut WebSocket) -> bool {
    let welcome_message = ServerMessage::Welcome;
    let welcome_text = serde_json::to_string(&welcome_message).expect("ServerMessage::Welcome has no fields, so serialization cannot fail");

    match socket.send(Message::Text(welcome_text.into())).await {
        Ok(()) => true,
        Err(e) => {
            eprintln!("failed to send welcome message: {e}");
            false
        }
}
}

async fn set_username(socket: &mut WebSocket) -> Option<String> {
    loop {
        match receive_client_message(socket).await {
            Received::Disconnected => return None,
            Received::Invalid => {
                send_error(socket, "invalid message format").await;
                continue;
            }
            Received::Message(ClientMessage::SetUsername { username }) => {
                if username.trim().is_empty() {
                    send_error(socket, "username cannot be empty").await;
                    continue;
                }
                return Some(username);
            }
            Received::Message(_) => {
                send_error(socket, "expected SetUsername message").await;
                continue;
            }
        }
    }
}

async fn confirm_username(socket: &mut WebSocket, username: &str) -> Option<bool> {
    let confirm_message = ServerMessage::ConfirmUsername {
        username: username.to_string(),
    };
    let confirm_text = serde_json::to_string(&confirm_message).unwrap();
    if socket.send(Message::Text(confirm_text.into())).await.is_err() {
        return None;
    }

    loop {
        match receive_client_message(socket).await {
            Received::Disconnected => return None,
            Received::Invalid => {
                send_error(socket, "invalid message format").await;
                continue;
            }
            Received::Message(ClientMessage::ConfirmUsername { confirmed }) => {
                return Some(confirmed);
            }
            Received::Message(_) => {
                send_error(socket, "expected ConfirmUsername message").await;
                continue;
            }
        }
    }
}

async fn get_confirmed_username(socket: &mut WebSocket) -> Option<String> {
    loop {
        let username = set_username(socket).await?;
        match confirm_username(socket, &username).await? {
            true => return Some(username),
            false => continue,
        }
    }
}

async fn select_room(socket: &mut WebSocket, state: AppState) -> Option<String> {
    let room_list: Vec<String> = state.rooms.iter().map(|entry| entry.key().clone()).collect();
    let room_list_message = ServerMessage::RoomList { rooms: room_list };
    let room_list_text = serde_json::to_string(&room_list_message).unwrap();
    if socket.send(Message::Text(room_list_text.into())).await.is_err() {
        return None;
    }

    loop {
        match receive_client_message(socket).await {
            Received::Disconnected => return None,
            Received::Invalid => {
                send_error(socket, "invalid message format").await;
                continue;
            }
            Received::Message(ClientMessage::JoinRoom { room }) => {
                if room.trim().is_empty() {
                    send_error(socket, "room name cannot be empty").await;
                    continue;
                }
                return Some(room);
            }
            Received::Message(_) => {
                send_error(socket, "expected JoinRoom message").await;
                continue;
            }
        }
    }
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    if !welcome_user(&mut socket).await {
        return;
    }

    let Some(username) = get_confirmed_username(&mut socket).await else {
        return;
    };

    let Some(room) = select_room(&mut socket, state.clone()).await else {
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
        let Ok(msg) = msg else {
            break;
        };
        let Message::Text(text) = msg else {
            continue; 
        };
        match serde_json::from_str::<ClientMessage>(&text) {
            Ok(ClientMessage::ChatMessage { message }) => {
                let reply = ServerMessage::ChatMessage {
                    username: username.clone(),
                    message,
                };
                let _ = tx.send(reply);
            }
            Ok(_) => {
                let _ = tx.send(ServerMessage::Error {
                    message: "unexpected message at this stage".to_string(),
                });
            }
            Err(_) => {
                let _ = tx.send(ServerMessage::Error {
                    message: "invalid message format".to_string(),
                });
            }
        }
    }

    let left = ServerMessage::LeftRoom {
        username: username.clone(),
    };
    let _ = tx.send(left);

    send_task.abort();
}