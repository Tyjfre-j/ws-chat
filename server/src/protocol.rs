pub use ws_chat_protocol::{ClientMessage, ErrorCode, ServerMessage};

pub enum Received {
    Message(ClientMessage),
    Ignored,
    Unsupported,
    Invalid,
    Disconnected,
}
