use ratatui::{
    DefaultTerminal, Frame,
    crossterm::event::{self, Event, KeyCode},
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Paragraph},
};

use futures_util::StreamExt;
use tokio_tungstenite::connect_async;

#[derive(Default)]
struct App {
    input: String,
    messages: Vec<String>,
}

async fn connect_and_listen() {
    let (ws_stream, _response) = connect_async("ws://127.0.0.1:3000/ws")
        .await
        .expect("failed to connect");

    let (_write, mut read) = ws_stream.split();

    while let Some(msg) = read.next().await {
        match msg {
            Ok(message) => eprintln!("received: {:?}", message),
            Err(e) => {
                eprintln!("error: {:?}", e);
                break;
            }
        }
    }

    eprintln!("connection closed");
}

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let runtime = tokio::runtime::Runtime::new()?;
    runtime.block_on(connect_and_listen());

    Ok(())
}

fn run(terminal: &mut DefaultTerminal, app: &mut App) -> std::io::Result<()> {
    loop {
        terminal.draw(|frame| render(frame, app))?;
        if let Event::Key(key) = event::read()? {
            match key.code {
                KeyCode::Esc => break Ok(()),
                KeyCode::Char(c) => app.input.push(c),
                KeyCode::Backspace => {
                    app.input.pop();
                }
                KeyCode::Enter => {
                    let msg = app.input.trim();
                    if msg.is_empty() {
                        continue;
                    }
                    app.messages.push(msg.to_string());
                    app.input.clear();
                }
                _ => {}
            }
        }
    }
}

fn render(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(1),
            Constraint::Length(3),
        ])
        .split(frame.area());

    frame.render_widget(
        Paragraph::new("Status: Disconnected")
            .block(Block::default().borders(Borders::ALL).title("ws-chat")),
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
