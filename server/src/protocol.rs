use serde::{Deserialize, Serialize};

#[derive(Serialize, Debug, Clone)]
#[serde(tag = "type", content = "data")]
pub enum ServerMessage {
    Welcome,
    ConfirmUsername { username: String },
    RoomList { rooms: Vec<String> },
    ChatMessage { username: String, message: String },
    JoinedRoom { username: String },
    LeftRoom { username: String },
    Error { code: ErrorCode, message: String },
}

#[derive(Serialize, Debug, Clone)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InvalidMessage,
    UnexpectedMessage,
    EmptyUsername,
    UsernameTooLong,
    UsernameTaken,
    EmptyRoom,
    RoomNameTooLong,
    UnsupportedMessage,
    MessageTooLarge,
    LaggedBehind,
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type", content = "data")]
pub enum ClientMessage {
    SetUsername { username: String },
    ConfirmUsername { confirmed: bool },
    JoinRoom { room: String },
    ChatMessage { message: String },
}

pub enum Received {
    Message(ClientMessage),
    Ignored,
    Invalid,
    Disconnected,
}
