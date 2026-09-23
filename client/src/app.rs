use crate::protocol::{ClientStage, ErrorCode, ServerMessage};
use crate::theme::ThemeName;

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
    pub max_message_scroll: usize,
    pub connected: bool,
    pub stage: ClientStage,
    pub total_messages_pushed: usize,
    pub theme: ThemeName,
    pub cached_full_text: String,
    pub cached_content_lines: usize,
    pub cached_width: u16,
    pub cached_generation: usize,
    pub cached_padded_text: String,
    pub cached_leading_padding: usize,
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
        self.message_scroll = self
            .message_scroll
            .saturating_add(1)
            .min(self.max_message_scroll);
    }

    pub fn scroll_messages_down(&mut self) {
        self.message_scroll = self.message_scroll.saturating_sub(1);
    }

    pub fn cycle_theme(&mut self) {
        self.theme = self.theme.next();
        self.push_event(ChatEvent::System(format!(
            "Theme switched to {}",
            self.theme.label()
        )));
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

        ServerMessage::ProposedUsername { username } => {
            app.push_event(ChatEvent::Prompt(format!(
                "Confirm username '{}'? (y/n)",
                username
            )));
            app.stage = ClientStage::ConfirmUsername {
                confirmed_username: username,
            };
        }

        ServerMessage::RoomList { rooms } => {
            let room_list = if rooms.is_empty() {
                "none yet, you can create one by typing its name".to_string()
            } else {
                rooms.join(", ")
            };
            app.push_event(ChatEvent::System(format!("Available rooms: {room_list}")));
            app.stage = ClientStage::SelectRoom;
        }

        ServerMessage::RoomJoinConfirmed { room } => {
            app.push_event(ChatEvent::System(format!("Joined room {room}")));
            app.stage = ClientStage::Chatting { room };
        }

        ServerMessage::ChatMessage { username, message } => {
            app.push_event(ChatEvent::Chat {
                username,
                text: message,
            });
        }

        ServerMessage::UserJoined { username } => {
            app.push_event(ChatEvent::Joined { username });
        }

        ServerMessage::UserLeft { username } => {
            app.push_event(ChatEvent::Left { username });
        }

        ServerMessage::Error { code, message } => {
            if matches!(app.stage, ClientStage::JoiningRoom { .. }) {
                app.stage = ClientStage::SelectRoom;
            } else {
                match code {
                    ErrorCode::EmptyUsername
                    | ErrorCode::UsernameTooLong
                    | ErrorCode::UsernameTaken => {
                        app.stage = ClientStage::SetUsername;
                    }

                    ErrorCode::EmptyChatMessage
                    | ErrorCode::MessageTooLarge
                    | ErrorCode::LaggedBehind
                    | ErrorCode::InvalidMessage
                    | ErrorCode::UnexpectedMessage
                    | ErrorCode::UnsupportedMessage
                    | ErrorCode::EmptyRoom
                    | ErrorCode::RoomNameTooLong => {}
                }
            }

            app.push_event(ChatEvent::Error(format!("Error from server: {}", message)));
        }
    }
}
