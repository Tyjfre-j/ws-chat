use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", content = "data")]
pub enum ClientMessage {
    SetUsername { username: String },
    AcceptUsername { accepted: bool },
    JoinRoom { room: String },
    ChatMessage { message: String },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", content = "data")]
pub enum ServerMessage {
    Welcome,
    ProposedUsername { username: String },
    RoomList { rooms: Vec<String> },
    RoomJoinConfirmed { room: String },
    ChatMessage { username: String, message: String },
    UserJoined { username: String },
    UserLeft { username: String },
    Error { code: ErrorCode, message: String },
}

#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InvalidMessage,
    UnexpectedMessage,
    EmptyUsername,
    UsernameTooLong,
    UsernameTaken,
    EmptyRoom,
    RoomNameTooLong,
    EmptyChatMessage,
    UnsupportedMessage,
    MessageTooLarge,
    LaggedBehind,
}

#[derive(Debug)]
pub enum Received<T> {
    Message(T),
    Ignored,
    Unsupported,
    Invalid,
    Disconnected,
}

pub const MAX_USERNAME_LEN: usize = 64;
pub const MAX_ROOM_NAME_LEN: usize = 64;
pub const MAX_CHAT_MESSAGE_LEN: usize = 4 * 1024;

pub fn has_control_characters(value: &str) -> bool {
    value.chars().any(char::is_control)
}
