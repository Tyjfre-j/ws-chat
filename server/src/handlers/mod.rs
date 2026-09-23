pub mod chat;
pub mod connection;
pub mod room;
pub mod username;

pub use connection::{handle_health, ws_handler};

use crate::net::send_error;
use crate::protocol::ErrorCode;
use connection::WsWrite;

pub(super) const MAX_MESSAGE_SIZE: usize = 64 * 1024;

pub(super) async fn notify_error(
    write: &mut WsWrite,
    code: ErrorCode,
    message: &str,
) -> Option<()> {
    if !send_error(write, code, message).await {
        tracing::warn!(?code, "failed to notify client; socket likely dead");
        return None;
    }
    Some(())
}
