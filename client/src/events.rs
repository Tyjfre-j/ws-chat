use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use futures_util::Sink;
use tokio_tungstenite::tungstenite::Message;

use crate::app::{App, ChatEvent};
use crate::net;
use crate::protocol::{
    ClientMessage, ClientStage, MAX_CHAT_MESSAGE_LEN, MAX_ROOM_NAME_LEN, MAX_USERNAME_LEN,
    has_control_characters,
};

pub enum KeyOutcome {
    Quit,
    Disconnected,
    Continue,
}

const MAX_INPUT_LEN: usize = 4 * 1024;

pub fn handle_local_key(app: &mut App, key: KeyEvent) -> KeyOutcome {
    if key.kind != KeyEventKind::Press {
        return KeyOutcome::Continue;
    }

    match key.code {
        KeyCode::Char('t') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            app.cycle_theme();
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
        KeyCode::Esc => return KeyOutcome::Quit,
        _ => {}
    }
    KeyOutcome::Continue
}

pub async fn handle_key<S>(app: &mut App, write: &mut S, key: KeyEvent) -> KeyOutcome
where
    S: Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    if key.kind != KeyEventKind::Press {
        return KeyOutcome::Continue;
    }

    if key.code == KeyCode::Enter {
        let input = std::mem::take(&mut app.input);
        return handle_enter(app, write, input).await;
    }

    handle_local_key(app, key)
}

fn mark_disconnected(app: &mut App, reason: &str) {
    app.connected = false;
    app.stage = ClientStage::Disconnected;
    app.push_event(ChatEvent::System(reason.to_string()));
}

async fn handle_enter<S>(app: &mut App, write: &mut S, input: String) -> KeyOutcome
where
    S: Sink<Message, Error = tokio_tungstenite::tungstenite::Error> + Unpin,
{
    match &app.stage {
        ClientStage::SetUsername => {
            let username = input.trim().to_string();

            if username.is_empty() {
                app.push_event(ChatEvent::Error("Username cannot be empty.".to_string()));
                return KeyOutcome::Continue;
            }

            if username.chars().count() > MAX_USERNAME_LEN {
                app.push_event(ChatEvent::Error("Username is too long.".to_string()));
                return KeyOutcome::Continue;
            }

            if has_control_characters(&username) {
                app.push_event(ChatEvent::Error(
                    "Username cannot contain control characters.".to_string(),
                ));
                return KeyOutcome::Continue;
            }

            let msg = ClientMessage::SetUsername { username };
            if !net::send_client_message(write, &msg).await {
                tracing::warn!("failed to send username; connection likely dead");
                app.input = input;
                mark_disconnected(app, "Failed to send username. Connection lost.");
                return KeyOutcome::Disconnected;
            }
        }
        ClientStage::ConfirmUsername { .. } => match input.trim().to_lowercase().as_str() {
            "y" => {
                let msg = ClientMessage::AcceptUsername { accepted: true };
                if !net::send_client_message(write, &msg).await {
                    tracing::warn!("failed to send username confirmation; connection likely dead");
                    mark_disconnected(app, "Failed to confirm username. Connection lost.");
                    return KeyOutcome::Disconnected;
                }
            }
            "n" => {
                let msg = ClientMessage::AcceptUsername { accepted: false };
                if !net::send_client_message(write, &msg).await {
                    tracing::warn!("failed to send username rejection; connection likely dead");
                    mark_disconnected(app, "Failed to send response. Connection lost.");
                    return KeyOutcome::Disconnected;
                }
                app.push_event(ChatEvent::System(
                    "Username not confirmed. Please enter a new username.".to_string(),
                ));
                app.stage = ClientStage::SetUsername;
            }
            _ => {
                app.push_event(ChatEvent::Error(
                    "Invalid input. Please enter 'y' or 'n'.".to_string(),
                ));
            }
        },
        ClientStage::SelectRoom => {
            let room = input.trim().to_string();

            if room.is_empty() {
                app.push_event(ChatEvent::Error("Room name cannot be empty.".to_string()));
                return KeyOutcome::Continue;
            }

            if room.chars().count() > MAX_ROOM_NAME_LEN {
                app.push_event(ChatEvent::Error("Room name is too long.".to_string()));
                return KeyOutcome::Continue;
            }

            if has_control_characters(&room) {
                app.push_event(ChatEvent::Error(
                    "Room name cannot contain control characters.".to_string(),
                ));
                return KeyOutcome::Continue;
            }

            let msg = ClientMessage::JoinRoom { room: room.clone() };
            if !net::send_client_message(write, &msg).await {
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
                app.push_event(ChatEvent::Error("Message cannot be empty.".to_string()));
                return KeyOutcome::Continue;
            }

            if message.len() > MAX_CHAT_MESSAGE_LEN {
                app.push_event(ChatEvent::Error("Message is too large.".to_string()));
                return KeyOutcome::Continue;
            }

            if has_control_characters(&message) {
                app.push_event(ChatEvent::Error(
                    "Message cannot contain control characters.".to_string(),
                ));
                return KeyOutcome::Continue;
            }

            let msg = ClientMessage::ChatMessage { message };
            if !net::send_client_message(write, &msg).await {
                tracing::warn!("failed to send chat message; connection likely dead");
                app.input = input;
                mark_disconnected(app, "Failed to send message. Connection lost.");
                return KeyOutcome::Disconnected;
            }
        }
        ClientStage::JoiningRoom { .. } => {
            app.push_event(ChatEvent::System(
                "Still waiting to join the room...".to_string(),
            ));
        }
        ClientStage::Connecting | ClientStage::Disconnected => {}
    }

    KeyOutcome::Continue
}

pub fn handle_retry_wait_key(app: &mut App, key: KeyEvent) -> KeyOutcome {
    if key.kind != KeyEventKind::Press {
        return KeyOutcome::Continue;
    }

    match key.code {
        KeyCode::Esc => KeyOutcome::Quit,
        KeyCode::Up => {
            app.scroll_messages_up();
            KeyOutcome::Continue
        }
        KeyCode::Down => {
            app.scroll_messages_down();
            KeyOutcome::Continue
        }
        _ => KeyOutcome::Continue,
    }
}
