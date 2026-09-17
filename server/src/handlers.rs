use axum::{
    extract::State,
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
};

use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;

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
    let error_text = serde_json::to_string(&error_reply)
        .expect("ServerMessage::Error shouldnt fail to serialize");

    if let Err(e) = socket.send(Message::Text(error_text.into())).await {
        eprintln!("failed to send error message: {e}");
    }
}

async fn receive_client_message(socket: &mut WebSocket) -> Received {
    let Some(msg) = socket.recv().await else {
        return Received::Disconnected;
    };
    let msg = match msg {
        Ok(msg) => msg,
        Err(e) => {
            eprintln!("error receiving message from client: {e}");
            return Received::Disconnected;
        }
    };

    let text = match msg {
        Message::Text(text) => text,
        Message::Close(_) => {
            return Received::Disconnected;
        }
        Message::Binary(_) => {
            return Received::Invalid;
        }
        _ => {
            return Received::Ignored;
        }
    };

    match serde_json::from_str::<ClientMessage>(&text) {
        Ok(parsed) => Received::Message(parsed),
        Err(e) => {
            eprintln!("error parsing client message: {e}");
            Received::Invalid
        }
    }
}

async fn welcome_user(socket: &mut WebSocket) -> bool {
    let welcome_message = ServerMessage::Welcome;
    let welcome_text = serde_json::to_string(&welcome_message)
        .expect("ServerMessage::Welcome shouldnt fail to serialize");

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
                let username = username.trim();
                if username.is_empty() {
                    send_error(socket, "username cannot be empty").await;
                    continue;
                }
                return Some(username.to_string());
            }
            Received::Ignored => {
                continue;
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
    let confirm_text = serde_json::to_string(&confirm_message)
        .expect("ServerMessage::ConfirmUsername shouldnt fail to serialize");

    if let Err(e) = socket.send(Message::Text(confirm_text.into())).await {
        eprintln!("failed to send confirm username message: {e}");
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
            Received::Ignored => {
                continue;
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
    let room_list: Vec<String> = state
        .rooms
        .iter()
        .map(|entry| entry.key().clone())
        .collect();
    let room_list_message = ServerMessage::RoomList { rooms: room_list };
    let room_list_text = serde_json::to_string(&room_list_message)
        .expect("ServerMessage::RoomList shouldnt fail to serialize");
    if let Err(e) = socket.send(Message::Text(room_list_text.into())).await {
        eprintln!("failed to send room list message: {e}");
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
                let room = room.trim();
                if room.is_empty() {
                    send_error(socket, "room name cannot be empty").await;
                    continue;
                }
                return Some(room.to_string());
            }
            Received::Ignored => {
                continue;
            }
            Received::Message(_) => {
                send_error(socket, "expected JoinRoom message").await;
                continue;
            }
        }
    }
}

async fn forward_broadcast_message(
    socket: &mut WebSocket,
    result: Result<ServerMessage, RecvError>,
) -> bool {
    match result {
        Ok(msg) => {
            let text = match serde_json::to_string(&msg) {
                Ok(text) => text,
                Err(e) => {
                    eprintln!("failed to serialize server message: {e}");
                    return false;
                }
            };
            socket.send(Message::Text(text.into())).await.is_ok()
        }
        Err(RecvError::Lagged(count)) => {
            eprintln!("lagged behind by {count} messages");
            true
        }
        Err(RecvError::Closed) => false,
    }
}

async fn handle_client_message(
    socket: &mut WebSocket,
    tx: &broadcast::Sender<ServerMessage>,
    username: &str,
    result: Option<Result<Message, axum::Error>>,
) -> bool {
    let Some(result) = result else {
        return false;
    };

    let msg = match result {
        Ok(msg) => msg,
        Err(e) => {
            eprintln!("error receiving message from client: {e}");
            return false;
        }
    };

    let text = match msg {
    Message::Text(text) => text,
    Message::Close(_) => return false, 
    Message::Binary(_) => {
        send_error(socket, "only text messages are supported").await;
        return true;
    }
    _ => return true, 
};

    match serde_json::from_str::<ClientMessage>(&text) {
        Ok(ClientMessage::ChatMessage { message }) => {
            let reply = ServerMessage::ChatMessage {
                username: username.to_string(),
                message,
            };

            let _ = tx.send(reply);
        }

        Ok(_) => {
            send_error(socket, "unexpected message at this stage").await;
        }

        Err(e) => {
            eprintln!("error parsing client message: {e}");
            send_error(socket, "invalid message format").await;
        }
    }

    true
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
        .or_insert_with(|| broadcast::channel(256).0)
        .clone();

    let mut rx = tx.subscribe();

    let joined = ServerMessage::JoinedRoom {
        username: username.clone(),
    };
    let _ = tx.send(joined);

    loop {
        tokio::select! {
            broadcast_result = rx.recv() => {
                if !forward_broadcast_message(&mut socket, broadcast_result).await {
                    break;
                }
            }
            client_result = socket.recv() => {
                if !handle_client_message(&mut socket, &tx, &username, client_result).await {
                    break;
                }
            }
        }
    }

    let left = ServerMessage::LeftRoom {
        username: username.clone(),
    };
    let _ = tx.send(left);
}
