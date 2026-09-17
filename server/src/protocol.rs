#[derive(serde::Serialize, Debug, Clone)]
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

#[derive(serde::Deserialize, Debug)]
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
