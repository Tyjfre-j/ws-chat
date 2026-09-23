pub use ws_chat_protocol::{
    ClientMessage, ErrorCode, MAX_CHAT_MESSAGE_LEN, MAX_ROOM_NAME_LEN, MAX_USERNAME_LEN, Received,
    ServerMessage, has_control_characters,
};

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

pub enum RunOutcome {
    Quit,
    ConnectionFailed,
    Disconnected,
    InputFailed,
}

pub enum RetryOutcome {
    Retry,
    Quit,
    InputFailed,
}
