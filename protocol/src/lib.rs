use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", content = "data")]
pub enum ClientMessage {
    SetUsername { username: String },
    ConfirmUsername { confirmed: bool },
    JoinRoom { room: String },
    ChatMessage { message: String },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(tag = "type", content = "data")]
pub enum ServerMessage {
    Welcome,
    ConfirmUsername { username: String },
    RoomList { rooms: Vec<String> },
    RoomJoined { room: String },
    ChatMessage { username: String, message: String },
    JoinedRoom { username: String },
    LeftRoom { username: String },
    Error { code: ErrorCode, message: String },
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
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
