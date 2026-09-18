use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Paragraph},
};

use ratatui::style::{Color, Style};

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
        ClientStage::Connecting => "Connecting".to_string(),
        ClientStage::SetUsername => "Entering Username".to_string(),
        ClientStage::ConfirmUsername { confirmed_username } => {
            format!("Confirming Username: {}", confirmed_username)
        }
        ClientStage::SelectRoom => "Selecting Room".to_string(),
        ClientStage::Chatting { room } => format!("Chatting in {}", room),
        ClientStage::Disconnected => "Disconnected".to_string(),
    };

    let border_color = match &app.stage {
        ClientStage::Connecting => Color::Yellow,
        ClientStage::Disconnected => Color::Red,
        ClientStage::Chatting { .. } => Color::Green,
        _ => Color::White,
    };

    frame.render_widget(
        Paragraph::new(status).block(
            Block::default()
                .borders(Borders::ALL)
                .title("ws-chat")
                .border_style(Style::default().fg(border_color)),
        ),
        chunks[0],
    );

    let visible_height = chunks[1].height.saturating_sub(2) as usize;
    let start = app.messages.len().saturating_sub(visible_height);
    let visible_messages = app.messages[start..].join("\n");

    frame.render_widget(
        Paragraph::new(visible_messages)
            .block(Block::default().borders(Borders::ALL).title("Messages")),
        chunks[1],
    );

    frame.render_widget(
        Paragraph::new(app.input.as_str())
            .block(Block::default().borders(Borders::ALL).title("Input")),
        chunks[2],
    );
}
