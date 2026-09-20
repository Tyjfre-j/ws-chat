use futures_util::{Sink, SinkExt};
use tokio_tungstenite::tungstenite::Message;

use crate::app::{App, handle_server_message};
use crate::protocol::ServerMessage;

pub async fn send_msg<S>(write: &mut S, msg: &crate::protocol::ClientMessage) -> bool
where
    S: Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    let json = serde_json::to_string(msg).expect("ClientMessage shouldn't fail to serialize");
    match write.send(Message::Text(json.into())).await {
        Ok(()) => true,
        Err(e) => {
            tracing::warn!(error = %e, message = ?msg, "failed to send message to server");
            false
        }
    }
}

pub fn handle_incoming(
    app: &mut App,
    server_msg: Option<Result<Message, tokio_tungstenite::tungstenite::Error>>,
) -> bool {
    match server_msg {
        Some(Ok(Message::Text(text))) => {
            match serde_json::from_str::<ServerMessage>(&text) {
                Ok(server_message) => handle_server_message(app, server_message),
                Err(e) => tracing::warn!(error = %e, raw = %text, "failed to parse server message"),
            }
            true
        }
        Some(Ok(Message::Close(frame))) => {
            tracing::info!(?frame, "server closed the connection");
            false
        }
        Some(Ok(Message::Ping(_) | Message::Pong(_))) => true,
        Some(Ok(Message::Binary(_))) => {
            tracing::warn!("received unsupported binary message from server");
            false
        }
        Some(Ok(other)) => {
            tracing::warn!(?other, "received unexpected message type from server");
            true
        }
        Some(Err(e)) => {
            tracing::warn!(error = %e, "error receiving message from server");
            false
        }
        None => {
            tracing::info!("connection closed");
            false
        }
    }
}
