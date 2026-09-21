use ratatui::{
    Frame,
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Paragraph, Wrap},
};

use crate::app::{App, ChatEvent};
use crate::protocol::ClientStage;
use ratatui::style::{Color, Style};
use ratatui::widgets::{BorderType, Padding};
use std::sync::OnceLock;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

const SINGLE_LINE_CONTENT: u16 = 1;
const BOX_BORDER: u16 = 1;
const SINGLE_LINE_BOX_HEIGHT: u16 = SINGLE_LINE_CONTENT + 2 * BOX_BORDER;
const SINGLE_LINE_PADDING: Padding = Padding::new(1, 1, 0, 0);
const MESSAGES_PADDING: Padding = Padding::uniform(1);
const FOOTER_HEIGHT: u16 = 1;

const BADGE_TEXTS: [&str; 3] = ["● Connected", "● Disconnected", "● Connecting"];

fn badge_width() -> u16 {
    static WIDTH: OnceLock<u16> = OnceLock::new();
    *WIDTH.get_or_init(|| {
        let max_text_width = BADGE_TEXTS
            .iter()
            .map(|s| s.width() as u16)
            .max()
            .unwrap_or(0);
        max_text_width + 2 * BOX_BORDER + SINGLE_LINE_PADDING.left + SINGLE_LINE_PADDING.right
    })
}

const MIN_TERMINAL_WIDTH: u16 = 40;

const MIN_MESSAGE_ROWS: u16 = 3;
const MESSAGES_CHROME: u16 = 2 * BOX_BORDER + 2;
const MIN_TERMINAL_HEIGHT: u16 = SINGLE_LINE_BOX_HEIGHT
    + MESSAGES_CHROME
    + MIN_MESSAGE_ROWS
    + SINGLE_LINE_BOX_HEIGHT
    + FOOTER_HEIGHT;

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

fn event_text(event: &ChatEvent) -> String {
    match event {
        ChatEvent::System(text) => text.clone(),
        ChatEvent::Prompt(text) => text.clone(),
        ChatEvent::Chat { username, text } => format!("[{username}]: {text}"),
        ChatEvent::Joined { username } => format!("{username} joined the room"),
        ChatEvent::Left { username } => format!("{username} left the room"),
        ChatEvent::Error(text) => text.clone(),
    }
}

fn wrapped_line_count(text: &str, width: u16) -> usize {
    Paragraph::new(text)
        .wrap(Wrap { trim: false })
        .line_count(width)
}

pub fn render(frame: &mut Frame, app: &mut App) {
    let area = frame.area();
    if area.width < MIN_TERMINAL_WIDTH || area.height < MIN_TERMINAL_HEIGHT {
        let message = format!(
            "Terminal too small ({}x{}).\nResize to at least {}x{}.",
            area.width, area.height, MIN_TERMINAL_WIDTH, MIN_TERMINAL_HEIGHT
        );
        frame.render_widget(
            Paragraph::new(message)
                .alignment(ratatui::layout::Alignment::Center)
                .wrap(Wrap { trim: true }),
            area,
        );
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(SINGLE_LINE_BOX_HEIGHT),
            Constraint::Min(1),
            Constraint::Length(SINGLE_LINE_BOX_HEIGHT),
            Constraint::Length(FOOTER_HEIGHT),
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

    let (badge_text, badge_color) = if app.connected {
        ("● Connected", Color::Green)
    } else if matches!(app.stage, ClientStage::Disconnected) {
        ("● Disconnected", Color::Red)
    } else {
        ("● Connecting", Color::Yellow)
    };

    let header_title = match &app.stage {
        ClientStage::Chatting { room } => format!("ws-chat — {room}"),
        _ => "ws-chat".to_string(),
    };

    let header_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(badge_width())])
        .split(chunks[0]);

    frame.render_widget(
        Paragraph::new(status).block(
            Block::default()
                .borders(Borders::ALL)
                .title(header_title)
                .border_style(Style::default().fg(border_color))
                .border_type(BorderType::Rounded)
                .padding(SINGLE_LINE_PADDING),
        ),
        header_chunks[0],
    );

    frame.render_widget(
        Paragraph::new(badge_text)
            .style(Style::default().fg(badge_color))
            .alignment(ratatui::layout::Alignment::Right)
            .wrap(Wrap { trim: false })
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(border_color))
                    .border_type(BorderType::Rounded)
                    .padding(SINGLE_LINE_PADDING),
            ),
        header_chunks[1],
    );

    let messages_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .padding(MESSAGES_PADDING);

    let messages_inner = messages_block.inner(chunks[1]);
    let visible_height = messages_inner.height as usize;
    let messages_width = messages_inner.width;

    let new_push_count = app
        .total_messages_pushed
        .saturating_sub(app.last_rendered_total_pushed)
        .min(app.messages.len());
    if new_push_count > 0 {
        if app.message_scroll > 0 {
            let new_messages = &app.messages[app.messages.len() - new_push_count..];
            let new_lines: usize = new_messages
                .iter()
                .map(|event| wrapped_line_count(&event_text(event), messages_width))
                .sum();
            app.message_scroll += new_lines;
        }
        app.last_rendered_total_pushed = app.total_messages_pushed;
    }

    let full_text = app
        .messages
        .iter()
        .map(event_text)
        .collect::<Vec<_>>()
        .join("\n");
    let content_lines = wrapped_line_count(&full_text, messages_width);

    let leading_padding = visible_height.saturating_sub(content_lines);
    let total_lines = content_lines + leading_padding;
    let padded_text = if leading_padding > 0 {
        format!("{}{}", "\n".repeat(leading_padding), full_text)
    } else {
        full_text
    };

    let max_scroll = total_lines.saturating_sub(visible_height);
    app.message_scroll = app.message_scroll.min(max_scroll);
    let scroll_offset = (max_scroll - app.message_scroll) as u16;

    let messages_title = if app.message_scroll == 0 {
        "Messages".to_string()
    } else {
        format!("Messages ({} newer; ↑/↓ to scroll)", app.message_scroll)
    };
    let messages_block = messages_block.title(messages_title);

    frame.render_widget(
        Paragraph::new(padded_text)
            .block(messages_block)
            .wrap(Wrap { trim: false })
            .scroll((scroll_offset, 0)),
        chunks[1],
    );

    let input_block = Block::default()
        .borders(Borders::ALL)
        .title("Input")
        .border_type(BorderType::Rounded)
        .padding(SINGLE_LINE_PADDING);

    let input_inner = input_block.inner(chunks[2]);
    let input_width = input_inner.width as usize;
    let visible_input = trailing_input(&app.input, input_width.saturating_sub(1));

    frame.render_widget(Paragraph::new(visible_input).block(input_block), chunks[2]);

    if matches!(
        app.stage,
        ClientStage::SetUsername
            | ClientStage::ConfirmUsername { .. }
            | ClientStage::SelectRoom
            | ClientStage::Chatting { .. }
    ) {
        let cursor_x = input_inner.x.saturating_add(visible_input.width() as u16);
        frame.set_cursor_position((cursor_x, input_inner.y));
    }

    let footer = Paragraph::new("Esc quit · ↑↓ scroll · Enter send")
        .style(Style::default().fg(Color::DarkGray));
    frame.render_widget(footer, chunks[3]);
}
