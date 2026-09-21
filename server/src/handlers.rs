use axum::{
    extract::State,
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
};

use tokio::sync::broadcast;
use tokio::sync::broadcast::error::RecvError;

use crate::protocol::{ClientMessage, ErrorCode, Received, ServerMessage};
use crate::state::AppState;

const MAX_MESSAGE_SIZE: usize = 64 * 1024;
const MAX_CHAT_MESSAGE_LEN: usize = 4 * 1024;
const MAX_USERNAME_LEN: usize = 64;
const MAX_ROOM_NAME_LEN: usize = 64;

fn has_control_characters(value: &str) -> bool {
    value.chars().any(char::is_control)
}

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
            tracing::warn!(error = %e, message = ?msg, "failed to send message to client");
            false
        }
    }
}

async fn send_error(socket: &mut WebSocket, code: ErrorCode, message: &str) -> bool {
    let error_reply = ServerMessage::Error {
        code,
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
            tracing::warn!(error = %e, "error receiving message from client");
            return Received::Disconnected;
        }
    };

    let text = match msg {
        Message::Text(text) => text,
        Message::Close(_) => {
            return Received::Disconnected;
        }
        Message::Binary(_) => return Received::Unsupported,
        _ => {
            return Received::Ignored;
        }
    };

    match serde_json::from_str::<ClientMessage>(&text) {
        Ok(parsed) => Received::Message(parsed),
        Err(e) => {
            tracing::warn!(error = %e, raw = %text, "failed to parse client message during setup");
            Received::Invalid
        }
    }
}

async fn set_username(socket: &mut WebSocket) -> Option<String> {
    loop {
        match receive_client_message(socket).await {
            Received::Disconnected => return None,
            Received::Unsupported => {
                if !send_error(
                    socket,
                    ErrorCode::UnsupportedMessage,
                    "only text messages are supported",
                )
                .await
                {
                    tracing::warn!(
                        "failed to notify client of unsupported message; socket likely dead"
                    );
                    return None;
                }
                continue;
            }
            Received::Invalid => {
                if !send_error(socket, ErrorCode::InvalidMessage, "invalid message format").await {
                    tracing::warn!(
                        "failed to notify client of invalid message; socket likely dead"
                    );
                    return None;
                }
                continue;
            }
            Received::Message(ClientMessage::SetUsername { username }) => {
                let username = username.trim();
                if username.is_empty() {
                    if !send_error(socket, ErrorCode::EmptyUsername, "username cannot be empty")
                        .await
                    {
                        tracing::warn!(
                            "failed to notify client of empty username; socket likely dead"
                        );
                        return None;
                    }
                    continue;
                }
                if username.chars().count() > MAX_USERNAME_LEN {
                    if !send_error(socket, ErrorCode::UsernameTooLong, "username is too long").await
                    {
                        tracing::warn!(
                            "failed to notify client of long username; socket likely dead"
                        );
                        return None;
                    }
                    continue;
                }
                if has_control_characters(username) {
                    if !send_error(
                        socket,
                        ErrorCode::InvalidMessage,
                        "username cannot contain control characters",
                    )
                    .await
                    {
                        return None;
                    }
                    continue;
                }
                return Some(username.to_string());
            }
            Received::Ignored => {
                continue;
            }
            Received::Message(_) => {
                if !send_error(
                    socket,
                    ErrorCode::UnexpectedMessage,
                    "expected SetUsername message",
                )
                .await
                {
                    tracing::warn!(
                        "failed to notify client of unexpected message; socket likely dead"
                    );
                    return None;
                }
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
            Received::Unsupported => {
                if !send_error(
                    socket,
                    ErrorCode::UnsupportedMessage,
                    "only text messages are supported",
                )
                .await
                {
                    tracing::warn!(
                        "failed to notify client of unsupported message; socket likely dead"
                    );
                    return None;
                }
                continue;
            }
            Received::Invalid => {
                if !send_error(socket, ErrorCode::InvalidMessage, "invalid message format").await {
                    tracing::warn!(
                        "failed to notify client of invalid message; socket likely dead"
                    );
                    return None;
                }
                continue;
            }
            Received::Message(ClientMessage::ConfirmUsername { confirmed }) => {
                return Some(confirmed);
            }
            Received::Ignored => {
                continue;
            }
            Received::Message(_) => {
                if !send_error(
                    socket,
                    ErrorCode::UnexpectedMessage,
                    "expected ConfirmUsername message",
                )
                .await
                {
                    tracing::warn!(
                        "failed to notify client of unexpected message; socket likely dead"
                    );
                    return None;
                }
                continue;
            }
        }
    }
}

async fn get_confirmed_username(socket: &mut WebSocket, state: &AppState) -> Option<String> {
    loop {
        let username = set_username(socket).await?;
        match confirm_username(socket, &username).await? {
            true => match state.usernames.entry(username.to_lowercase()) {
                dashmap::mapref::entry::Entry::Occupied(_) => {
                    if !send_error(
                        socket,
                        ErrorCode::UsernameTaken,
                        "username is already taken",
                    )
                    .await
                    {
                        tracing::warn!(
                            "failed to notify client of taken username; socket likely dead"
                        );
                        return None;
                    }
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
            Received::Unsupported => {
                if !send_error(
                    socket,
                    ErrorCode::UnsupportedMessage,
                    "only text messages are supported",
                )
                .await
                {
                    tracing::warn!(
                        "failed to notify client of unsupported message; socket likely dead"
                    );
                    return None;
                }
                continue;
            }
            Received::Invalid => {
                if !send_error(socket, ErrorCode::InvalidMessage, "invalid message format").await {
                    tracing::warn!(
                        "failed to notify client of invalid message; socket likely dead"
                    );
                    return None;
                }
                continue;
            }
            Received::Message(ClientMessage::JoinRoom { room }) => {
                let room = room.trim();
                if room.is_empty() {
                    if !send_error(socket, ErrorCode::EmptyRoom, "room name cannot be empty").await
                    {
                        tracing::warn!(
                            "failed to notify client of empty room name; socket likely dead"
                        );
                        return None;
                    }
                    continue;
                }
                if room.chars().count() > MAX_ROOM_NAME_LEN {
                    if !send_error(socket, ErrorCode::RoomNameTooLong, "room name is too long")
                        .await
                    {
                        tracing::warn!(
                            "failed to notify client of long room name; socket likely dead"
                        );
                        return None;
                    }
                    continue;
                }
                if has_control_characters(room) {
                    if !send_error(
                        socket,
                        ErrorCode::InvalidMessage,
                        "room name cannot contain control characters",
                    )
                    .await
                    {
                        return None;
                    }
                    continue;
                }
                return Some(room.to_lowercase());
            }
            Received::Ignored => {
                continue;
            }
            Received::Message(_) => {
                if !send_error(
                    socket,
                    ErrorCode::UnexpectedMessage,
                    "expected JoinRoom message",
                )
                .await
                {
                    tracing::warn!(
                        "failed to notify client of unexpected message; socket likely dead"
                    );
                    return None;
                }
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
            tracing::warn!(lagged_by = count, "client lagged behind broadcast channel");
            send_server_message(
                socket,
                &ServerMessage::Error {
                    code: ErrorCode::LaggedBehind,
                    message: format!("You missed {count} messages due to lag."),
                },
            )
            .await
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
            tracing::warn!(error = %e, "error receiving message from client");
            return false;
        }
    };

    let text = match msg {
        Message::Text(text) => text,
        Message::Close(_) => return false,
        Message::Binary(_) => {
            if !send_error(
                socket,
                ErrorCode::UnsupportedMessage,
                "only text messages are supported",
            )
            .await
            {
                tracing::warn!(
                    "failed to notify client of unsupported message; socket likely dead"
                );
                return false;
            }
            return true;
        }
        _ => return true,
    };

    match serde_json::from_str::<ClientMessage>(&text) {
        Ok(ClientMessage::ChatMessage { message }) => {
            let message = message.trim().to_string();
            if message.len() > MAX_CHAT_MESSAGE_LEN {
                if !send_error(socket, ErrorCode::MessageTooLarge, "message too large").await {
                    tracing::warn!(
                        "failed to notify client of oversized message; socket likely dead"
                    );
                    return false;
                }
                return true;
            }
            if message.is_empty() {
                if !send_error(
                    socket,
                    ErrorCode::EmptyChatMessage,
                    "message cannot be empty",
                )
                .await
                {
                    tracing::warn!("failed to notify client of empty message; socket likely dead");
                    return false;
                }
                return true;
            }
            if has_control_characters(&message) {
                if !send_error(
                    socket,
                    ErrorCode::InvalidMessage,
                    "message cannot contain control characters",
                )
                .await
                {
                    tracing::warn!(
                        "failed to notify client of invalid message; socket likely dead"
                    );
                    return false;
                }
                return true;
            }
            let reply = ServerMessage::ChatMessage {
                username: username.to_string(),
                message,
            };
            let _ = tx.send(reply);
        }
        Ok(_) => {
            if !send_error(
                socket,
                ErrorCode::UnexpectedMessage,
                "unexpected message at this stage",
            )
            .await
            {
                tracing::warn!("failed to notify client of unexpected message; socket likely dead");
                return false;
            }
        }
        Err(e) => {
            tracing::warn!(error = %e, raw = %text, "failed to parse client message during chat");
            if !send_error(socket, ErrorCode::InvalidMessage, "invalid message format").await {
                tracing::warn!("failed to notify client of invalid message; socket likely dead");
                return false;
            }
        }
    }

    true
}

async fn run_chat_loop(socket: &mut WebSocket, state: &AppState, username: String, room: String) {
    let (tx, mut rx) = {
        let entry = state
            .rooms
            .entry(room.clone())
            .or_insert_with(|| broadcast::channel(256).0);
        let tx = entry.value().clone();
        let rx = tx.subscribe();
        (tx, rx)
    };

    if !send_server_message(socket, &ServerMessage::RoomJoined { room: room.clone() }).await {
        drop(rx);
        state
            .rooms
            .remove_if(&room, |_, tx: &broadcast::Sender<ServerMessage>| {
                tx.receiver_count() == 0
            });
        return;
    }

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
        state.usernames.remove(&username.to_lowercase());
        return;
    };

    run_chat_loop(&mut socket, &state, username.clone(), room).await;

    state.usernames.remove(&username.to_lowercase());
}
