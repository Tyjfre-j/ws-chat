use futures_util::{Sink, SinkExt};
use tokio_tungstenite::tungstenite::Message;

use crate::protocol::{ClientMessage, Received, ServerMessage};

pub async fn send_client_message<S>(write: &mut S, msg: &ClientMessage) -> bool
where
    S: Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    let json = match serde_json::to_string(msg) {
        Ok(json) => json,
        Err(e) => {
            tracing::warn!(error = %e, message = ?msg, "failed to serialize client message");
            return false;
        }
    };

    match write.send(Message::Text(json.into())).await {
        Ok(()) => true,
        Err(e) => {
            tracing::warn!(error = %e, message = ?msg, "failed to send message to server");
            false
        }
    }
}

pub fn receive_server_message(
    stream_item: Option<Result<Message, tokio_tungstenite::tungstenite::Error>>,
) -> Received<ServerMessage> {
    match stream_item {
        Some(Ok(Message::Text(text))) => match serde_json::from_str::<ServerMessage>(&text) {
            Ok(server_message) => Received::Message(server_message),
            Err(e) => {
                tracing::warn!(error = %e, raw = %text, "failed to parse server message");
                Received::Invalid
            }
        },
        Some(Ok(Message::Close(frame))) => {
            tracing::info!(?frame, "server closed the connection");
            Received::Disconnected
        }
        Some(Ok(Message::Binary(_))) => {
            tracing::warn!("received unsupported binary message from server");
            Received::Unsupported
        }
        Some(Ok(_other)) => Received::Ignored,
        Some(Err(e)) => {
            tracing::warn!(error = %e, "error receiving message from server");
            Received::Disconnected
        }
        None => {
            tracing::info!("server connection closed");
            Received::Disconnected
        }
    }
}
