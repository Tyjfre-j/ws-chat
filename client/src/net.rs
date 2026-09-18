use futures_util::{Sink, SinkExt};
use tokio_tungstenite::tungstenite::Message;

use crate::app::{App, handle_server_message};
use crate::protocol::ServerMessage;

pub async fn send_msg<S>(
    write: &mut S,
    msg: &crate::protocol::ClientMessage,
) -> Result<(), tokio_tungstenite::tungstenite::Error>
where
    S: Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    let json = serde_json::to_string(msg).expect("ClientMessage shouldn't fail to serialize");
    write.send(Message::Text(json.into())).await
}

pub fn handle_incoming(
    app: &mut App,
    server_msg: Option<Result<Message, tokio_tungstenite::tungstenite::Error>>,
) -> bool {
    match server_msg {
        Some(Ok(message)) => {
            if let Ok(text) = message.to_text() {
                match serde_json::from_str::<ServerMessage>(text) {
                    Ok(server_message) => handle_server_message(app, server_message),
                    Err(e) => eprintln!("failed to parse server message: {e}"),
                }
            } else {
                eprintln!("received non-text message from server");
            }
            true
        }
        Some(Err(e)) => {
            eprintln!("error: {e:?}");
            false
        }
        None => {
            eprintln!("connection closed");
            false
        }
    }
}
