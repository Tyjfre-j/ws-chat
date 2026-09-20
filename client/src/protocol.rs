pub use ws_chat_protocol::{ClientMessage, ErrorCode, ServerMessage};

#[derive(Default)]
pub enum ClientStage {
    #[default]
    Connecting,
    SetUsername,
    ConfirmUsername {
        confirmed_username: String,
    },
    SelectRoom,
    JoiningRoom {
        room: String,
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
