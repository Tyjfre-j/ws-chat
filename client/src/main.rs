mod protocol;

use ratatui::{
    DefaultTerminal, Frame,
    crossterm::event::{self, Event, KeyCode},
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

enum AppEvent {
    Server(ServerMessage),
    Disconnected,
}

async fn connect_and_listen(tx: tokio::sync::mpsc::Sender<AppEvent>) {
    let (ws_stream, _response) = connect_async("ws://127.0.0.1:3000/ws")
        .await
        .expect("failed to connect");

    let (_write, mut read) = ws_stream.split();

    while let Some(message) = read.next().await {
        match message {
            Ok(message) => {
                if let Ok(text) = message.to_text() {
                    match serde_json::from_str::<ServerMessage>(text) {
                        Ok(server_message) => {
                            if let Err(e) = tx.send(AppEvent::Server(server_message)).await {
                                eprintln!("failed to send server message to main thread: {e}");
                                break;
                            }
                        }
                        Err(e) => {
                            eprintln!("failed to parse server message: {e}");
                        }
                    }
                } else {
                    eprintln!("received non-text message from server");
                }
            }
            Err(e) => {
                eprintln!("error: {:?}", e);
                break;
            }
        }
    }

    eprintln!("connection closed");
    let _ = tx.send(AppEvent::Disconnected).await;
}

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;

    let (tx, mut rx) = tokio::sync::mpsc::channel::<AppEvent>(32);

    let runtime = tokio::runtime::Runtime::new()?;
    runtime.spawn(connect_and_listen(tx));

    let mut app = App::default();
    ratatui::run(|terminal| run(terminal, &mut app, &mut rx))?;

    Ok(())
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

fn run(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    rx: &mut tokio::sync::mpsc::Receiver<AppEvent>,
) -> std::io::Result<()> {
    loop {
        terminal.draw(|frame| render(frame, app))?;

        while let Ok(event) = rx.try_recv() {
            match event {
                AppEvent::Server(msg) => {
                    handle_server_message(app, msg);
                }
                AppEvent::Disconnected => {
                    app.connected = false;
                    app.messages.push("Disconnected from server".to_string());
                    app.stage = ClientStage::Disconnected;
                }
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
