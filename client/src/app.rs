use crate::protocol::{ClientStage, ServerMessage};

#[derive(Default)]
pub struct App {
    pub input: String,
    pub messages: Vec<String>,
    pub connected: bool,
    pub stage: ClientStage,
}

pub fn handle_server_message(app: &mut App, msg: ServerMessage) {
    match msg {
        ServerMessage::Welcome => {
            app.connected = true;
            app.messages.push("Connected to the server".to_string());
            app.stage = ClientStage::SetUsername;
        }
        ServerMessage::ConfirmUsername { username } => {
            app.messages
                .push(format!("Confirm username '{}'? (y/n)", username));
            app.stage = ClientStage::ConfirmUsername { proposed: username };
        }
        ServerMessage::RoomList { rooms } => {
            app.messages.push(format!("Available rooms: {:?}", rooms));
            app.stage = ClientStage::SelectRoom { rooms };
        }
        ServerMessage::ChatMessage { username, message } => {
            app.messages.push(format!("[{}]: {}", username, message));
        }
        ServerMessage::JoinedRoom { username } => {
            app.messages.push(format!("{} joined the room", username));
        }
        ServerMessage::LeftRoom { username } => {
            app.messages.push(format!("{} left the room", username));
        }
        ServerMessage::Error { message } => {
            app.messages.push(format!("Error from server: {}", message));
        }
    }
}
