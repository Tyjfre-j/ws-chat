use serde::{Deserialize, Serialize};

#[derive(Serialize, Debug)]
#[serde(tag = "type", content = "data")]
pub enum ClientMessage {
    SetUsername { username: String },
    ConfirmUsername { confirmed: bool },
    JoinRoom { room: String },
    ChatMessage { message: String },
}

#[derive(Deserialize, Debug)]
#[serde(tag = "type", content = "data")]
pub enum ServerMessage {
    Welcome,
    ConfirmUsername { username: String },
    RoomList { rooms: Vec<String> },
    ChatMessage { username: String, message: String },
    JoinedRoom { username: String },
    LeftRoom { username: String },
    Error { message: String },
}

#[derive(Default)]
pub enum ClientStage {
    #[default]
    Connecting,
    SetUsername,
    ConfirmUsername {
        confirmed_username: String,
    },
    SelectRoom {
        rooms: Vec<String>,
    },
    Chatting {
        room: String,
    },
    Disconnected,
}

pub enum ConnectionOutcome {
    Quit,
    FailedToConnect,
    Disconnected,
}
