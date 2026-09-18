use axum::{
    extract::State,
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
};

use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;

use crate::protocol::{ClientMessage, Received, ServerMessage};
use crate::state::AppState;

const MAX_MESSAGE_SIZE: usize = 64 * 1024;

pub async fn handle_health() -> &'static str {
    "OK"
}

pub async fn ws_handler(State(state): State<AppState>, ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.max_message_size(MAX_MESSAGE_SIZE)
        .on_upgrade(move |socket| handle_socket(socket, state))
}

async fn send_server_message(socket: &mut WebSocket, msg: &ServerMessage) -> bool {
    let text = serde_json::to_string(msg).expect("ServerMessage shouldnt fail to serialize");
    match socket.send(Message::Text(text.into())).await {
        Ok(()) => true,
        Err(e) => {
            eprintln!("failed to send message ({msg:?}): {e}");
            false
        }
    }
}

async fn send_error(socket: &mut WebSocket, message: &str) -> bool {
    let error_reply = ServerMessage::Error {
        message: message.to_string(),
    };
    send_server_message(socket, &error_reply).await
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
        eprintln!("failed to parse client message during setup");
        eprintln!("  received: {text:?}");
        eprintln!("  error: {e}");
        Received::Invalid
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
    if !send_server_message(
        socket,
        &ServerMessage::ConfirmUsername {
            username: username.to_string(),
        },
    )
    .await
    {
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

async fn get_confirmed_username(socket: &mut WebSocket, state: &AppState) -> Option<String> {
    loop {
        let username = set_username(socket).await?;
        match confirm_username(socket, &username).await? {
            true => match state.usernames.entry(username.clone()) {
                dashmap::mapref::entry::Entry::Occupied(_) => {
                    send_error(socket, "username is already taken").await;
                    continue;
                }
                dashmap::mapref::entry::Entry::Vacant(entry) => {
                    entry.insert(());
                    return Some(username);
                }
            },
            false => continue,
        }
    }
}

async fn select_room(socket: &mut WebSocket, state: &AppState) -> Option<String> {
    let room_list: Vec<String> = state
        .rooms
        .iter()
        .map(|entry| entry.key().clone())
        .collect();

    if !send_server_message(socket, &ServerMessage::RoomList { rooms: room_list }).await {
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
        Ok(msg) => send_server_message(socket, &msg).await,
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
            eprintln!("failed to parse client message during chat");
            eprintln!("  received: {text:?}");
            eprintln!("  error: {e}");
            send_error(socket, "invalid message format").await;
        }
    }

    true
}

async fn run_chat_loop(
    socket: &mut WebSocket,
    state: &AppState,
    username: String,
    room: String,
) {
    // Subscribe while holding the shard lock, so a concurrent room-removal
    // can't slip in between "room exists" and "we subscribed to it".
    let (tx, mut rx) = {
        let entry = state
            .rooms
            .entry(room.clone())
            .or_insert_with(|| broadcast::channel(256).0);
        let tx = entry.get().clone();
        let rx = tx.subscribe();
        (tx, rx)
    };

    let _ = tx.send(ServerMessage::JoinedRoom {
        username: username.clone(),
    });

    loop {
        tokio::select! {
            broadcast_result = rx.recv() => {
                if !forward_broadcast_message(socket, broadcast_result).await {
                    break;
                }
            }
            client_result = socket.recv() => {
                if !handle_client_message(socket, &tx, &username, client_result).await {
                    break;
                }
            }
        }
    }

    let _ = tx.send(ServerMessage::LeftRoom {
        username: username.clone(),
    });

    // Only now is this client truly gone from the room.
    drop(rx);
    state
        .rooms
        .remove_if(&room, |_, tx: &broadcast::Sender<ServerMessage>| {
            tx.receiver_count() == 0
        });
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    if !send_server_message(&mut socket, &ServerMessage::Welcome).await {
        return;
    }

    let Some(username) = get_confirmed_username(&mut socket, &state).await else {
        return;
    };

    let Some(room) = select_room(&mut socket, &state).await else {
        state.usernames.remove(&username);
        return;
    };

    run_chat_loop(&mut socket, &state, username.clone(), room).await;

    state.usernames.remove(&username);
}