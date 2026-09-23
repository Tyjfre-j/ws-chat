use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Padding, Paragraph, Wrap},
};

use crate::app::{App, ChatEvent};
use crate::protocol::ClientStage;
use crate::theme::theme_for;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

const STATUS_HEIGHT: u16 = 1;
const RULE_HEIGHT: u16 = 1;
const INPUT_HEIGHT: u16 = 1;
const HINTS_HEIGHT: u16 = 1;
const MIN_MESSAGE_ROWS: u16 = 3;
const MIN_TERMINAL_HEIGHT: u16 = STATUS_HEIGHT
    + RULE_HEIGHT
    + MIN_MESSAGE_ROWS
    + RULE_HEIGHT
    + INPUT_HEIGHT
    + RULE_HEIGHT
    + HINTS_HEIGHT;

const MESSAGES_MARGIN: Padding = Padding::new(2, 2, 0, 0);
const HINTS: &str = "Esc quit · ↑↓ scroll · Enter send · Ctrl+T theme";

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
        ChatEvent::Chat { username, text } => format!("{username}  {text}"),
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
    let theme = theme_for(app.theme);

    let area = frame.area();

    frame.render_widget(
        Block::default().style(Style::default().bg(theme.background)),
        area,
    );

    let min_terminal_width = HINTS
        .width()
        .max((MESSAGES_MARGIN.left + MESSAGES_MARGIN.right + 1) as usize)
        as u16;

    if area.width < min_terminal_width || area.height < MIN_TERMINAL_HEIGHT {
        let message = format!(
            "Terminal too small ({}x{}).\nResize to at least {}x{}.",
            area.width, area.height, min_terminal_width, MIN_TERMINAL_HEIGHT
        );

        frame.render_widget(
            Paragraph::new(message)
                .alignment(Alignment::Center)
                .wrap(Wrap { trim: true }),
            area,
        );

        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(STATUS_HEIGHT),
            Constraint::Length(RULE_HEIGHT),
            Constraint::Min(1),
            Constraint::Length(RULE_HEIGHT),
            Constraint::Length(INPUT_HEIGHT),
            Constraint::Length(RULE_HEIGHT),
            Constraint::Length(HINTS_HEIGHT),
        ])
        .split(area);

    let status_area = chunks[0];
    let rule_area_1 = chunks[1];
    let messages_area = chunks[2];
    let rule_area_2 = chunks[3];
    let input_area = chunks[4];
    let rule_area_3 = chunks[5];
    let hints_area = chunks[6];

    let messages_block = Block::default().padding(MESSAGES_MARGIN);
    let messages_inner = messages_block.inner(messages_area);
    let visible_height = messages_inner.height as usize;
    let messages_width = messages_inner.width;

    let was_at_top = app.max_message_scroll > 0 && app.message_scroll >= app.max_message_scroll;

    let new_push_count = app
        .total_messages_pushed
        .saturating_sub(app.cached_generation)
        .min(app.messages.len());

    let cache_stale =
        app.cached_generation != app.total_messages_pushed || app.cached_width != messages_width;

    if cache_stale {
        app.cached_full_text = app
            .messages
            .iter()
            .map(event_text)
            .collect::<Vec<_>>()
            .join("\n");

        app.cached_content_lines = wrapped_line_count(&app.cached_full_text, messages_width);
        app.cached_width = messages_width;
    }

    let content_lines = app.cached_content_lines;

    let leading_padding = visible_height.saturating_sub(content_lines);
    let total_lines = content_lines + leading_padding;

    if cache_stale || app.cached_leading_padding != leading_padding {
        app.cached_padded_text = if leading_padding > 0 {
            format!("{}{}", "\n".repeat(leading_padding), app.cached_full_text)
        } else {
            app.cached_full_text.clone()
        };
        app.cached_leading_padding = leading_padding;
    }

    let max_scroll = total_lines.saturating_sub(visible_height);

    if new_push_count > 0 && app.message_scroll > 0 {
        let new_messages = &app.messages[app.messages.len() - new_push_count..];

        let new_lines: usize = new_messages
            .iter()
            .map(|event| wrapped_line_count(&event_text(event), messages_width))
            .sum();

        app.message_scroll = app.message_scroll.saturating_add(new_lines).min(max_scroll);
    }

    if was_at_top {
        app.message_scroll = max_scroll;
    }

    app.max_message_scroll = max_scroll;
    app.message_scroll = app.message_scroll.min(max_scroll);
    app.cached_generation = app.total_messages_pushed;

    let scroll_offset = (max_scroll - app.message_scroll).min(u16::MAX as usize) as u16;

    let stage_text = match &app.stage {
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

    let (state_label, state_color) = if app.connected {
        ("Connected", theme.connected)
    } else if matches!(app.stage, ClientStage::Disconnected) {
        ("Disconnected", theme.disconnected)
    } else {
        ("Connecting", theme.connecting)
    };

    let status_line = Line::from(vec![
        Span::styled("●", Style::default().fg(state_color)),
        Span::styled(
            format!(" {state_label} · {stage_text}"),
            Style::default().fg(theme.text),
        ),
    ]);

    let scroll_hint = if app.message_scroll > 0 {
        format!("{} newer ↑↓", app.message_scroll)
    } else {
        String::new()
    };

    let scroll_hint_width = scroll_hint.width() as u16;

    let status_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Min(1), Constraint::Length(scroll_hint_width)])
        .split(status_area);

    frame.render_widget(Paragraph::new(status_line), status_chunks[0]);

    frame.render_widget(
        Paragraph::new(scroll_hint)
            .style(Style::default().fg(theme.dim))
            .alignment(Alignment::Right),
        status_chunks[1],
    );

    let rule = "─".repeat(area.width as usize);
    let rule_style = Style::default().fg(theme.rule);

    frame.render_widget(Paragraph::new(rule.as_str()).style(rule_style), rule_area_1);

    frame.render_widget(Paragraph::new(rule.as_str()).style(rule_style), rule_area_2);

    frame.render_widget(Paragraph::new(rule).style(rule_style), rule_area_3);

    frame.render_widget(
        Paragraph::new(app.cached_padded_text.as_str())
            .style(Style::default().fg(theme.text))
            .block(messages_block)
            .wrap(Wrap { trim: false })
            .scroll((scroll_offset, 0)),
        messages_area,
    );

    let prefix = "› ";
    let prefix_width = prefix.width();
    let input_width = input_area.width as usize;

    let available_width = input_width.saturating_sub(prefix_width).saturating_sub(1);

    let visible_input = trailing_input(&app.input, available_width);

    let input_line = Line::from(vec![
        Span::styled(prefix, Style::default().fg(state_color)),
        Span::styled(visible_input, Style::default().fg(theme.text)),
    ]);

    frame.render_widget(Paragraph::new(input_line), input_area);

    if matches!(
        app.stage,
        ClientStage::SetUsername
            | ClientStage::ConfirmUsername { .. }
            | ClientStage::SelectRoom
            | ClientStage::Chatting { .. }
    ) {
        let cursor_x = input_area
            .x
            .saturating_add(prefix_width as u16)
            .saturating_add(visible_input.width() as u16);

        frame.set_cursor_position((cursor_x, input_area.y));
    }

    frame.render_widget(
        Paragraph::new(HINTS)
            .style(Style::default().fg(theme.dim))
            .alignment(Alignment::Center),
        hints_area,
    );
}
