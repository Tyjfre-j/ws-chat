use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Paragraph},
};

use ratatui::style::{Color, Style};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::app::App;
use crate::protocol::ClientStage;

fn trailing_input(input: &str, max_width: usize) -> &str {
    let mut width = 0;
    let mut start = input.len();

    for (index, character) in input.char_indices().rev() {
        let character_width = character.width().unwrap_or(0);
        if width + character_width > max_width {
            break;
        }
        width += character_width;
        start = index;
    }

    &input[start..]
}

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
        ClientStage::JoiningRoom { room } => format!("Joining {room}"),
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
    let end = app.messages.len().saturating_sub(app.message_scroll);
    let start = end.saturating_sub(visible_height);
    let visible_messages = app.messages[start..end].join("\n");

    let messages_title = if app.message_scroll == 0 {
        "Messages".to_string()
    } else {
        format!("Messages ({} newer; ↑/↓ to scroll)", app.message_scroll)
    };

    frame.render_widget(
        Paragraph::new(visible_messages)
            .block(Block::default().borders(Borders::ALL).title(messages_title)),
        chunks[1],
    );

    let input_width = chunks[2].width.saturating_sub(2) as usize;
    let visible_input = trailing_input(&app.input, input_width);

    frame.render_widget(
        Paragraph::new(visible_input).block(Block::default().borders(Borders::ALL).title("Input")),
        chunks[2],
    );

    if matches!(
        app.stage,
        ClientStage::SetUsername
            | ClientStage::ConfirmUsername { .. }
            | ClientStage::SelectRoom
            | ClientStage::Chatting { .. }
    ) {
        let cursor_x = chunks[2]
            .x
            .saturating_add(1)
            .saturating_add(visible_input.width().min(input_width.saturating_sub(1)) as u16);
        frame.set_cursor_position((cursor_x, chunks[2].y.saturating_add(1)));
    }
}
