use axum::extract::ws::Message;
use futures_util::StreamExt;
use tokio::sync::broadcast;

use crate::net::{forward_broadcast_message, receive_client_message, send_server_message};
use crate::protocol::{
    ClientMessage, ErrorCode, MAX_CHAT_MESSAGE_LEN, Received, ServerMessage, has_control_characters,
};
use crate::state::AppState;

use super::{
    connection::{WsRead, WsWrite},
    notify_error,
};

pub(super) async fn handle_client_message(
    write: &mut WsWrite,
    tx: &broadcast::Sender<ServerMessage>,
    username: &str,
    stream_item: Option<Result<Message, axum::Error>>,
) -> bool {
    match receive_client_message(stream_item) {
        Received::Disconnected => false,

        Received::Unsupported => notify_error(
            write,
            ErrorCode::UnsupportedMessage,
            "only text messages are supported",
        )
        .await
        .is_some(),

        Received::Invalid => {
            notify_error(write, ErrorCode::InvalidMessage, "invalid message format")
                .await
                .is_some()
        }

        Received::Ignored => true,

        Received::Message(ClientMessage::ChatMessage { message }) => {
            let message = message.trim().to_string();

            if message.len() > MAX_CHAT_MESSAGE_LEN {
                return notify_error(write, ErrorCode::MessageTooLarge, "message too large")
                    .await
                    .is_some();
            }

            if message.is_empty() {
                return notify_error(
                    write,
                    ErrorCode::EmptyChatMessage,
                    "message cannot be empty",
                )
                .await
                .is_some();
            }

            if has_control_characters(&message) {
                return notify_error(
                    write,
                    ErrorCode::InvalidMessage,
                    "message cannot contain control characters",
                )
                .await
                .is_some();
            }

            let reply = ServerMessage::ChatMessage {
                username: username.to_string(),
                message,
            };

            let _ = tx.send(reply);

            true
        }

        Received::Message(_) => notify_error(
            write,
            ErrorCode::UnexpectedMessage,
            "unexpected message at this stage",
        )
        .await
        .is_some(),
    }
}

pub(super) async fn run_chat_loop(
    write: &mut WsWrite,
    read: &mut WsRead,
    state: &AppState,
    username: String,
    room: String,
) {
    let (tx, mut rx) = {
        let entry = state
            .rooms
            .entry(room.clone())
            .or_insert_with(|| broadcast::channel(256).0);

        let tx = entry.value().clone();
        let rx = tx.subscribe();

        (tx, rx)
    };

    if !send_server_message(
        write,
        &ServerMessage::RoomJoinConfirmed { room: room.clone() },
    )
    .await
    {
        drop(rx);

        state
            .rooms
            .remove_if(&room, |_, tx: &broadcast::Sender<ServerMessage>| {
                tx.receiver_count() == 0
            });

        return;
    }

    let _ = tx.send(ServerMessage::UserJoined {
        username: username.clone(),
    });

    loop {
        tokio::select! {
            broadcast_result = rx.recv() => {
                if !forward_broadcast_message(write, broadcast_result).await {
                    break;
                }
            }

            client_result = read.next() => {
                if !handle_client_message(
                    write,
                    &tx,
                    &username,
                    client_result,
                )
                .await
                {
                    break;
                }
            }
        }
    }

    let _ = tx.send(ServerMessage::UserLeft {
        username: username.clone(),
    });

    drop(rx);

    state
        .rooms
        .remove_if(&room, |_, tx: &broadcast::Sender<ServerMessage>| {
            tx.receiver_count() == 0
        });
}
