use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Paragraph},
};

use crate::app::App;
use crate::protocol::ClientStage;
pub fn render(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(frame.area());

    let status = match &app.stage {
        ClientStage::Connecting => "Connecting",
        ClientStage::SetUsername => "Entering Username",
        ClientStage::ConfirmUsername { .. } => "Confirming Username",
        ClientStage::SelectRoom { .. } => "Selecting Room",
        ClientStage::Chatting { .. } => "Chatting",
        ClientStage::Disconnected => "Disconnected",
    };

    frame.render_widget(
        Paragraph::new(status).block(Block::default().borders(Borders::ALL).title("ws-chat")),
        chunks[0],
    );

    frame.render_widget(
        Paragraph::new(app.messages.join("\n"))
            .block(Block::default().borders(Borders::ALL).title("Messages")),
        chunks[1],
    );

    frame.render_widget(
        Paragraph::new(app.input.as_str())
            .block(Block::default().borders(Borders::ALL).title("Input")),
        chunks[2],
    );
}