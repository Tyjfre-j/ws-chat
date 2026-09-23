use axum::extract::ws::Message;
use futures_util::{Sink, SinkExt};

use tokio::sync::broadcast::error::RecvError;

use crate::protocol::{ClientMessage, ErrorCode, Received, ServerMessage};

pub async fn send_server_message<S>(write: &mut S, msg: &ServerMessage) -> bool
where
    S: Sink<Message, Error = axum::Error> + Unpin,
{
    let text = match serde_json::to_string(msg) {
        Ok(text) => text,
        Err(e) => {
            tracing::warn!(error = %e, message = ?msg, "failed to serialize server message");
            return false;
        }
    };

    match write.send(Message::Text(text.into())).await {
        Ok(()) => true,
        Err(e) => {
            tracing::warn!(error = %e, message = ?msg, "failed to send message to client");
            false
        }
    }
}

pub async fn send_error<S>(write: &mut S, code: ErrorCode, message: &str) -> bool
where
    S: Sink<Message, Error = axum::Error> + Unpin,
{
    let error_reply = ServerMessage::Error {
        code,
        message: message.to_string(),
    };

    send_server_message(write, &error_reply).await
}

pub fn receive_client_message(
    stream_item: Option<Result<Message, axum::Error>>,
) -> Received<ClientMessage> {
    match stream_item {
        Some(Ok(Message::Text(text))) => match serde_json::from_str::<ClientMessage>(&text) {
            Ok(parsed) => Received::Message(parsed),
            Err(e) => {
                tracing::warn!(error = %e, raw = %text, "failed to parse client message");
                Received::Invalid
            }
        },
        Some(Ok(Message::Close(frame))) => {
            tracing::info!(?frame, "client closed the connection");
            Received::Disconnected
        }
        Some(Ok(Message::Binary(_))) => {
            tracing::warn!("received unsupported binary message from client");
            Received::Unsupported
        }
        Some(Ok(_other)) => Received::Ignored,
        Some(Err(e)) => {
            tracing::warn!(error = %e, "error receiving message from client");
            Received::Disconnected
        }
        None => {
            tracing::info!("client connection closed");
            Received::Disconnected
        }
    }
}

pub async fn forward_broadcast_message<S>(
    write: &mut S,
    result: Result<ServerMessage, RecvError>,
) -> bool
where
    S: Sink<Message, Error = axum::Error> + Unpin,
{
    match result {
        Ok(msg) => send_server_message(write, &msg).await,

        Err(RecvError::Lagged(count)) => {
            tracing::warn!(lagged_by = count, "client lagged behind broadcast channel");

            send_error(
                write,
                ErrorCode::LaggedBehind,
                &format!("You missed {count} messages due to lag."),
            )
            .await
        }

        Err(RecvError::Closed) => {
            tracing::info!("broadcast channel closed");
            false
        }
    }
}
