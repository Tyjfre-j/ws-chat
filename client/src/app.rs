use crate::protocol::{ClientStage, ServerMessage};

#[derive(Default)]
pub struct App {
    pub input: String,
    pub messages: Vec<String>,
    pub connected: bool,
    pub stage: ClientStage,
}

impl App {
    const MAX_MESSAGES: usize = 200;

    pub fn push_message(&mut self, message: String) {
        self.messages.push(message);
        if self.messages.len() > Self::MAX_MESSAGES {
            self.messages.remove(0);
        }
    }
}

pub fn handle_server_message(app: &mut App, msg: ServerMessage) {
    match msg {
        ServerMessage::Welcome => {
            app.connected = true;
            app.push_message(
                "Connected to the server please provide the username u wanna go with:".to_string(),
            );
            app.stage = ClientStage::SetUsername;
        }
        ServerMessage::ConfirmUsername { username } => {
            app.push_message(format!("Confirm username '{}'? (y/n)", username));
            app.stage = ClientStage::ConfirmUsername {
                confirmed_username: username,
            };
        }
        ServerMessage::RoomList { rooms } => {
            app.push_message(format!("Available rooms: {:?}", rooms));
            app.stage = ClientStage::SelectRoom;
        }
        ServerMessage::ChatMessage { username, message } => {
            app.push_message(format!("[{}]: {}", username, message));
        }
        ServerMessage::JoinedRoom { username } => {
            app.push_message(format!("{} joined the room", username));
        }
        ServerMessage::LeftRoom { username } => {
            app.push_message(format!("{} left the room", username));
        }
        ServerMessage::Error { message } => {
            app.push_message(format!("Error from server: {}", message));
        }
    }
}
