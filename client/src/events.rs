use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use futures_util::Sink;
use tokio_tungstenite::tungstenite::Message;

use crate::app::App;
use crate::net;
use crate::protocol::{ClientMessage, ClientStage};

pub enum KeyOutcome {
    Quit,
    Disconnected,
    Continue,
}

const MAX_INPUT_LEN: usize = 4 * 1024;

pub async fn handle_key<S>(app: &mut App, write: &mut S, key: KeyEvent) -> KeyOutcome
where
    S: Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    if key.kind != KeyEventKind::Press {
        return KeyOutcome::Continue;
    }

    match key.code {
        KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            return KeyOutcome::Quit;
        }
        KeyCode::Char(c)
            if !key
                .modifiers
                .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
        {
            if app.input.len() + c.len_utf8() <= MAX_INPUT_LEN {
                app.input.push(c);
            }
        }
        KeyCode::Backspace => {
            app.input.pop();
        }
        KeyCode::Up => app.scroll_messages_up(),
        KeyCode::Down => app.scroll_messages_down(),
        KeyCode::Enter => {
            let input = std::mem::take(&mut app.input);
            return handle_enter(app, write, input).await;
        }
        KeyCode::Esc => return KeyOutcome::Quit,
        _ => {}
    }
    KeyOutcome::Continue
}

fn mark_disconnected(app: &mut App, reason: &str) {
    app.connected = false;
    app.stage = ClientStage::Disconnected;
    app.push_message(reason.to_string());
}

async fn handle_enter<S>(app: &mut App, write: &mut S, input: String) -> KeyOutcome
where
    S: Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    match &app.stage {
        ClientStage::SetUsername => {
            let msg = ClientMessage::SetUsername {
                username: input.clone(),
            };
            if !net::send_msg(write, &msg).await {
                tracing::warn!("failed to send username; connection likely dead");
                app.input = input;
                mark_disconnected(app, "Failed to send username. Connection lost.");
                return KeyOutcome::Disconnected;
            }
        }
        ClientStage::ConfirmUsername { .. } => match input.trim().to_lowercase().as_str() {
            "y" => {
                let msg = ClientMessage::ConfirmUsername { confirmed: true };
                if !net::send_msg(write, &msg).await {
                    tracing::warn!("failed to send username confirmation; connection likely dead");
                    mark_disconnected(app, "Failed to confirm username. Connection lost.");
                    return KeyOutcome::Disconnected;
                }
            }
            "n" => {
                let msg = ClientMessage::ConfirmUsername { confirmed: false };
                if !net::send_msg(write, &msg).await {
                    tracing::warn!("failed to send username rejection; connection likely dead");
                    mark_disconnected(app, "Failed to send response. Connection lost.");
                    return KeyOutcome::Disconnected;
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
        ClientStage::SelectRoom => {
            let room = input.trim().to_string();
            if room.is_empty() {
                app.push_message("Room name cannot be empty.".to_string());
                return KeyOutcome::Continue;
            }

            let msg = ClientMessage::JoinRoom { room: room.clone() };
            if !net::send_msg(write, &msg).await {
                tracing::warn!("failed to send join room request; connection likely dead");
                app.input = input;
                mark_disconnected(app, "Failed to join room. Connection lost.");
                return KeyOutcome::Disconnected;
            } else {
                app.stage = ClientStage::JoiningRoom { room };
            }
        }
        ClientStage::Chatting { .. } => {
            let message = input.trim().to_string();
            if message.is_empty() {
                app.push_message("Message cannot be empty.".to_string());
                return KeyOutcome::Continue;
            }
            let msg = ClientMessage::ChatMessage { message };
            if !net::send_msg(write, &msg).await {
                tracing::warn!("failed to send chat message; connection likely dead");
                app.input = input;
                mark_disconnected(app, "Failed to send message. Connection lost.");
                return KeyOutcome::Disconnected;
            }
        }
        ClientStage::JoiningRoom { .. } => {
            app.push_message("Still waiting to join the room...".to_string());
        }
        ClientStage::Connecting | ClientStage::Disconnected => {}
    }

    KeyOutcome::Continue
}
