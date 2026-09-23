use futures_util::StreamExt;

use crate::net::{receive_client_message, send_server_message};
use crate::protocol::{
    ClientMessage, ErrorCode, MAX_ROOM_NAME_LEN, Received, ServerMessage, has_control_characters,
};
use crate::state::AppState;

use super::{
    connection::{WsRead, WsWrite},
    notify_error,
};

pub(super) async fn select_room(
    write: &mut WsWrite,
    read: &mut WsRead,
    state: &AppState,
) -> Option<String> {
    let room_list: Vec<String> = state
        .rooms
        .iter()
        .map(|entry| entry.key().clone())
        .collect();

    if !send_server_message(write, &ServerMessage::RoomList { rooms: room_list }).await {
        return None;
    }

    loop {
        match receive_client_message(read.next().await) {
            Received::Disconnected => return None,

            Received::Unsupported => {
                notify_error(
                    write,
                    ErrorCode::UnsupportedMessage,
                    "only text messages are supported",
                )
                .await?;
            }

            Received::Invalid => {
                notify_error(write, ErrorCode::InvalidMessage, "invalid message format").await?;
            }

            Received::Message(ClientMessage::JoinRoom { room }) => {
                let room = room.trim();

                if room.is_empty() {
                    notify_error(write, ErrorCode::EmptyRoom, "room name cannot be empty").await?;
                    continue;
                }

                if room.chars().count() > MAX_ROOM_NAME_LEN {
                    notify_error(write, ErrorCode::RoomNameTooLong, "room name is too long")
                        .await?;
                    continue;
                }

                if has_control_characters(room) {
                    notify_error(
                        write,
                        ErrorCode::InvalidMessage,
                        "room name cannot contain control characters",
                    )
                    .await?;
                    continue;
                }

                return Some(room.to_lowercase());
            }

            Received::Ignored => {}

            Received::Message(_) => {
                notify_error(
                    write,
                    ErrorCode::UnexpectedMessage,
                    "expected JoinRoom message",
                )
                .await?;
            }
        }
    }
}
