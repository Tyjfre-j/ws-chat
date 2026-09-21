use crate::protocol::{ClientStage, ErrorCode, ServerMessage};

pub enum ChatEvent {
    System(String),
    Prompt(String),
    Chat { username: String, text: String },
    Joined { username: String },
    Left { username: String },
    Error(String),
}

#[derive(Default)]
pub struct App {
    pub input: String,
    pub messages: Vec<ChatEvent>,
    pub message_scroll: usize,
    pub connected: bool,
    pub stage: ClientStage,
    pub total_messages_pushed: usize,
    pub last_rendered_total_pushed: usize,
}

impl App {
    const MAX_MESSAGES: usize = 200;

    pub fn push_event(&mut self, event: ChatEvent) {
        self.messages.push(event);
        self.total_messages_pushed += 1;
        if self.messages.len() > Self::MAX_MESSAGES {
            self.messages.remove(0);
        }
    }

    pub fn scroll_messages_up(&mut self) {
        self.message_scroll = self.message_scroll.saturating_add(1);
    }

    pub fn scroll_messages_down(&mut self) {
        self.message_scroll = self.message_scroll.saturating_sub(1);
    }
}

pub fn handle_server_message(app: &mut App, msg: ServerMessage) {
    match msg {
        ServerMessage::Welcome => {
            app.connected = true;
            app.push_event(ChatEvent::System(
                "Connected to the server please provide the username u wanna go with:".to_string(),
            ));
            app.stage = ClientStage::SetUsername;
        }
        ServerMessage::ConfirmUsername { username } => {
            app.push_event(ChatEvent::Prompt(format!(
                "Confirm username '{}'? (y/n)",
                username
            )));
            app.stage = ClientStage::ConfirmUsername {
                confirmed_username: username,
            };
        }
        ServerMessage::RoomList { rooms } => {
            app.push_event(ChatEvent::System(format!("Available rooms: {:?}", rooms)));
            app.stage = ClientStage::SelectRoom;
        }
        ServerMessage::RoomJoined { room } => {
            app.push_event(ChatEvent::System(format!("Joined room {room}")));
            app.stage = ClientStage::Chatting { room };
        }
        ServerMessage::ChatMessage { username, message } => {
            app.push_event(ChatEvent::Chat {
                username,
                text: message,
            });
        }
        ServerMessage::JoinedRoom { username } => {
            app.push_event(ChatEvent::Joined { username });
        }
        ServerMessage::LeftRoom { username } => {
            app.push_event(ChatEvent::Left { username });
        }
        ServerMessage::Error { code, message } => {
            match code {
                ErrorCode::UsernameTaken => {
                    app.stage = ClientStage::SetUsername;
                }
                _ => {
                    if matches!(app.stage, ClientStage::JoiningRoom { .. }) {
                        app.stage = ClientStage::SelectRoom;
                    }
                }
            }
            app.push_event(ChatEvent::Error(format!("Error from server: {}", message)));
        }
    }
}
