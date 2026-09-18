mod protocol;

use crossterm::event::EventStream;

use ratatui::{
    DefaultTerminal, Frame,
    layout::{Constraint, Direction, Layout},
    widgets::{Block, Borders, Paragraph},
};

use futures_util::StreamExt;
use protocol::{ClientStage, ServerMessage};
use serde_json;
use tokio_tungstenite::connect_async;

#[derive(Default)]
struct App {
    input: String,
    messages: Vec<String>,
    connected: bool,
    stage: ClientStage,
}

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let runtime = tokio::runtime::Runtime::new()?;

    let mut app = App::default();
    let mut terminal = ratatui::init();
    let result = runtime.block_on(run(&mut terminal, &mut app));
    ratatui::restore();

    result.map_err(Into::into)
}

fn handle_server_message(app: &mut App, msg: ServerMessage) {
    match msg {
        ServerMessage::Welcome => {
            app.connected = true;
            app.messages.push("Connected to server".to_string());
            app.stage = ClientStage::SetUsername;
        }
        ServerMessage::ConfirmUsername { username } => {
            app.messages
                .push(format!("Confirm username '{}'? (y/n)", username));
            app.stage = ClientStage::ConfirmUsername { proposed: username };
        }
        ServerMessage::RoomList { rooms } => {
            app.messages.push(format!("Available rooms: {:?}", rooms));
            app.stage = ClientStage::SelectRoom { rooms };
        }
        ServerMessage::ChatMessage { username, message } => {
            app.messages.push(format!("[{}]: {}", username, message));
        }
        ServerMessage::JoinedRoom { username } => {
            app.messages.push(format!("{} joined the room", username));
        }
        ServerMessage::LeftRoom { username } => {
            app.messages.push(format!("{} left the room", username));
        }
        ServerMessage::Error { message } => {
            app.messages.push(format!("Error from server: {}", message));
        }
    }
}

async fn run(terminal: &mut DefaultTerminal, app: &mut App) -> std::io::Result<()> {
    let (ws_stream, _response) = connect_async("ws://127.0.0.1:3000/ws")
        .await
        .expect("failed to connect");

    let (write, mut read) = ws_stream.split();
    let mut events = EventStream::new();

    loop {
        terminal.draw(|frame| render(frame, app))?;

        tokio::select! {
            server_msg = read.next() => {
               match server_msg {
                    Some(Ok(message)) => {
                        if let Ok(text) = message.to_text() {
                            match serde_json::from_str::<ServerMessage>(text) {
                                Ok(server_message) => handle_server_message(app, server_message),
                                Err(e) => eprintln!("failed to parse server message: {e}"),
                            }
                        } else {
                            eprintln!("received non-text message from server");
                        }
                    }
                    Some(Err(e)) => {
                        eprintln!("error: {:?}", e);
                        break;
                    }
                    None => {
                        eprintln!("connection closed");
                        break;
                    }
                }
            }
            key_event = events.next() => {
                match key_event {
                    Some(Ok(crossterm::event::Event::Key(key))) => {
                        match key.code {
                            crossterm::event::KeyCode::Char(c) => {
                                app.input.push(c);
                            }
                            crossterm::event::KeyCode::Backspace => {
                                app.input.pop();
                            }
                            crossterm::event::KeyCode::Enter => {
                                app.messages.push(format!("You: {}", app.input));
                                app.input.clear();
                            }
                            crossterm::event::KeyCode::Esc => return Ok(()),
                            _ => {}
                        }
                    }
                    Some(Ok(_)) => {}
                    Some(Err(e)) => eprintln!("error reading key event: {:?}", e),
                    None => break,
                }
            }
        }
    }

    Ok(())
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
