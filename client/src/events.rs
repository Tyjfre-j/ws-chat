use crossterm::event::{KeyCode, KeyEvent};
use futures_util::Sink;
use tokio_tungstenite::tungstenite::Message;

use crate::app::App;
use crate::net;
use crate::protocol::{ClientMessage, ClientStage};

pub enum KeyOutcome {
    Quit,
    Continue,
}

pub async fn handle_key<S>(app: &mut App, write: &mut S, key: KeyEvent) -> KeyOutcome
where
    S: Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    match key.code {
        KeyCode::Char(c) => app.input.push(c),
        KeyCode::Backspace => {
            app.input.pop();
        }
        KeyCode::Enter => {
            let input = std::mem::take(&mut app.input);
            handle_enter(app, write, input).await;
        }
        KeyCode::Esc => return KeyOutcome::Quit,
        _ => {}
    }
    KeyOutcome::Continue
}

async fn handle_enter<S>(app: &mut App, write: &mut S, input: String)
where
    S: Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    match &app.stage {
        ClientStage::SetUsername => {
            let msg = ClientMessage::SetUsername {
                username: input.clone(),
            };
            if let Err(e) = net::send_msg(write, &msg).await {
                eprintln!("failed to send username: {e}");
                app.input = input;
            }
        }
        ClientStage::ConfirmUsername { .. } => match input.trim().to_lowercase().as_str() {
            "y" => {
                let msg = ClientMessage::ConfirmUsername { confirmed: true };
                if let Err(e) = net::send_msg(write, &msg).await {
                    eprintln!("failed to send username confirmation: {e}");
                }
            }
            "n" => {
                let msg = ClientMessage::ConfirmUsername { confirmed: false };
                if let Err(e) = net::send_msg(write, &msg).await {
                    eprintln!("failed to send username rejection: {e}");
                }
                app.push_message(
                    "Username not confirmed. Please enter a new username.".to_string(),
                );
                app.stage = ClientStage::SetUsername;
            }
            _ => {
                app.push_message("Invalid input. Please enter 'y' or 'n'.".to_string());
            }
        },
        ClientStage::SelectRoom { rooms } => {
            if rooms.contains(&input) {
                let msg = ClientMessage::JoinRoom {
                    room: input.clone(),
                };
                if let Err(e) = net::send_msg(write, &msg).await {
                    eprintln!("failed to send join room request: {e}");
                } else {
                    app.stage = ClientStage::Chatting { room: input };
                }
            } else {
                app.push_message(format!(
                    "Room '{input}' does not exist. Please select a valid room."
                ));
            }
        }
        ClientStage::Chatting { .. } => {
            let msg = ClientMessage::ChatMessage { message: input };
            if let Err(e) = net::send_msg(write, &msg).await {
                eprintln!("failed to send chat message: {e}");
            }
        }
        ClientStage::Connecting | ClientStage::Disconnected => {
            // no live connection to send on; ignore input
        }
    }
}
