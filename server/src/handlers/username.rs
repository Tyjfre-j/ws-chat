use crate::net::{receive_client_message, send_server_message};
use crate::protocol::{
    ClientMessage, ErrorCode, MAX_USERNAME_LEN, Received, ServerMessage, has_control_characters,
};
use crate::state::AppState;
use futures_util::StreamExt;

use super::{
    connection::{WsRead, WsWrite},
    notify_error,
};

pub(super) async fn set_username(write: &mut WsWrite, read: &mut WsRead) -> Option<String> {
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

            Received::Message(ClientMessage::SetUsername { username }) => {
                let username = username.trim();

                if username.is_empty() {
                    notify_error(write, ErrorCode::EmptyUsername, "username cannot be empty")
                        .await?;
                    continue;
                }

                if username.chars().count() > MAX_USERNAME_LEN {
                    notify_error(write, ErrorCode::UsernameTooLong, "username is too long").await?;
                    continue;
                }

                if has_control_characters(username) {
                    notify_error(
                        write,
                        ErrorCode::InvalidMessage,
                        "username cannot contain control characters",
                    )
                    .await?;
                    continue;
                }

                return Some(username.to_string());
            }

            Received::Ignored => {}

            Received::Message(_) => {
                notify_error(
                    write,
                    ErrorCode::UnexpectedMessage,
                    "expected SetUsername message",
                )
                .await?;
            }
        }
    }
}

pub(super) async fn confirm_username(
    write: &mut WsWrite,
    read: &mut WsRead,
    username: &str,
) -> Option<bool> {
    if !send_server_message(
        write,
        &ServerMessage::ProposedUsername {
            username: username.to_string(),
        },
    )
    .await
    {
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

            Received::Message(ClientMessage::AcceptUsername { accepted }) => {
                return Some(accepted);
            }

            Received::Ignored => {}

            Received::Message(_) => {
                notify_error(
                    write,
                    ErrorCode::UnexpectedMessage,
                    "expected AcceptUsername message",
                )
                .await?;
            }
        }
    }
}

enum Reservation {
    Confirmed,
    Taken,
}

pub(super) async fn get_confirmed_username(
    write: &mut WsWrite,
    read: &mut WsRead,
    state: &AppState,
) -> Option<String> {
    loop {
        let username = set_username(write, read).await?;

        if confirm_username(write, read, &username).await? {
            let outcome = match state.usernames.entry(username.to_lowercase()) {
                dashmap::mapref::entry::Entry::Occupied(_) => Reservation::Taken,
                dashmap::mapref::entry::Entry::Vacant(entry) => {
                    entry.insert(());
                    Reservation::Confirmed
                }
            };

            match outcome {
                Reservation::Confirmed => return Some(username),
                Reservation::Taken => {
                    notify_error(write, ErrorCode::UsernameTaken, "username is already taken")
                        .await?;
                }
            }
        }
    }
}
