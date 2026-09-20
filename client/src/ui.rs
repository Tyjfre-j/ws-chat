use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Paragraph},
};

use crate::app::App;
use crate::protocol::ClientStage;
use ratatui::style::{Color, Style};
use ratatui::widgets::{BorderType, Padding};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

const SINGLE_LINE_CONTENT: u16 = 1;
const BOX_BORDER: u16 = 1;
const SINGLE_LINE_BOX_HEIGHT: u16 = SINGLE_LINE_CONTENT + 2 * BOX_BORDER;
const SINGLE_LINE_PADDING: Padding = Padding::new(1, 1, 0, 0);
const MESSAGES_PADDING: Padding = Padding::uniform(1);

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
            Constraint::Length(SINGLE_LINE_BOX_HEIGHT),
            Constraint::Min(1),
            Constraint::Length(SINGLE_LINE_BOX_HEIGHT),
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
                .border_style(Style::default().fg(border_color))
                .border_type(BorderType::Rounded)
                .padding(SINGLE_LINE_PADDING),
        ),
        chunks[0],
    );

    let messages_title = if app.message_scroll == 0 {
        "Messages".to_string()
    } else {
        format!("Messages ({} newer; ↑/↓ to scroll)", app.message_scroll)
    };

    let messages_block = Block::default()
        .borders(Borders::ALL)
        .title(messages_title)
        .border_type(BorderType::Rounded)
        .padding(MESSAGES_PADDING);

    let messages_inner = messages_block.inner(chunks[1]);
    let visible_height = messages_inner.height as usize;

    let end = app.messages.len().saturating_sub(app.message_scroll);
    let start = end.saturating_sub(visible_height);
    let visible_messages = app.messages[start..end].join("\n");

    frame.render_widget(
        Paragraph::new(visible_messages).block(messages_block),
        chunks[1],
    );

    let input_block = Block::default()
        .borders(Borders::ALL)
        .title("Input")
        .border_type(BorderType::Rounded)
        .padding(SINGLE_LINE_PADDING);

    let input_inner = input_block.inner(chunks[2]);
    let input_width = input_inner.width as usize;
    let visible_input = trailing_input(&app.input, input_width);

    frame.render_widget(Paragraph::new(visible_input).block(input_block), chunks[2]);

    if matches!(
        app.stage,
        ClientStage::SetUsername
            | ClientStage::ConfirmUsername { .. }
            | ClientStage::SelectRoom
            | ClientStage::Chatting { .. }
    ) {
        let cursor_x = input_inner
            .x
            .saturating_add(visible_input.width().min(input_width.saturating_sub(1)) as u16);
        frame.set_cursor_position((cursor_x, input_inner.y));
    }
}
