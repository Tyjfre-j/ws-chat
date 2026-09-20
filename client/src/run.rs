use crossterm::event::EventStream;
use futures_util::{SinkExt, StreamExt};
use ratatui::DefaultTerminal;
use tokio_tungstenite::connect_async;

use crate::app::App;
use crate::events::{self, KeyOutcome};
use crate::net;
use crate::protocol::ClientStage;
use crate::protocol::ConnectionOutcome;

const DEFAULT_SERVER_URL: &str = "ws://127.0.0.1:3000/ws";

pub async fn run_connection(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    events_stream: &mut EventStream,
) -> std::io::Result<ConnectionOutcome> {
    app.connected = false;
    app.input.clear();
    app.stage = ClientStage::Connecting;
    terminal.draw(|frame| crate::ui::render(frame, app))?;

    let server_url =
        std::env::var("WS_CHAT_SERVER_URL").unwrap_or_else(|_| DEFAULT_SERVER_URL.to_string());
    let ws_stream = match connect_async(&server_url).await {
        Ok((ws_stream, _response)) => ws_stream,
        Err(e) => {
            app.connected = false;
            app.push_message(format!("Failed to connect: {e}"));
            app.stage = ClientStage::Disconnected;
            return Ok(ConnectionOutcome::FailedToConnect);
        }
    };

    let (mut write, mut read) = ws_stream.split();

    loop {
        terminal.draw(|frame| crate::ui::render(frame, app))?;

        tokio::select! {
            server_msg = read.next() => {
                let server_msg = match server_msg {
                    Some(Ok(tokio_tungstenite::tungstenite::Message::Ping(payload))) => {
                        if let Err(error) = write.send(tokio_tungstenite::tungstenite::Message::Pong(payload)).await {
                            tracing::warn!(%error, "failed to respond to server ping");
                            break;
                        }
                        continue;
                    }
                    message => message,
                };
                if !net::handle_incoming(app, server_msg) {
                    break;
                }
            }
            key_event = events_stream.next() => {
                match key_event {
                    Some(Ok(crossterm::event::Event::Key(key)))
                        if key.kind == crossterm::event::KeyEventKind::Press => {
                        match events::handle_key(app, &mut write, key).await {
                            KeyOutcome::Quit => {
                                let _ = write.close().await;
                                return Ok(ConnectionOutcome::Quit);
                            }
                            KeyOutcome::Disconnected => break,
                            KeyOutcome::Continue => {}
                        }
                    }
                    Some(Ok(_)) => {}
                    Some(Err(e)) => tracing::warn!(error = ?e, "error reading key event"),
                    None => break,
                }
            }
        }
    }

    app.connected = false;
    app.push_message("Disconnected from server".to_string());
    app.stage = ClientStage::Disconnected;

    Ok(ConnectionOutcome::Disconnected)
}

pub async fn run(terminal: &mut DefaultTerminal, app: &mut App) -> std::io::Result<()> {
    let mut events_stream = EventStream::new();
    let mut backoff = std::time::Duration::from_secs(1);

    loop {
        match run_connection(terminal, app, &mut events_stream).await? {
            ConnectionOutcome::Quit => break,
            ConnectionOutcome::Disconnected => {
                backoff = std::time::Duration::from_secs(1);
                app.push_message(format!("Reconnecting in {}s...", backoff.as_secs()));
                if wait_before_retry(terminal, app, &mut events_stream, backoff).await? {
                    return Ok(());
                }
                backoff = (backoff * 2).min(std::time::Duration::from_secs(30));
            }
            ConnectionOutcome::FailedToConnect => {
                app.push_message(format!("Retrying in {}s...", backoff.as_secs()));
                if wait_before_retry(terminal, app, &mut events_stream, backoff).await? {
                    return Ok(());
                }
                backoff = (backoff * 2).min(std::time::Duration::from_secs(30));
            }
        }
    }

    Ok(())
}

async fn wait_before_retry(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    events_stream: &mut EventStream,
    backoff: std::time::Duration,
) -> std::io::Result<bool> {
    terminal.draw(|frame| crate::ui::render(frame, app))?;

    let retry_timer = tokio::time::sleep(backoff);
    tokio::pin!(retry_timer);

    loop {
        tokio::select! {
            _ = &mut retry_timer => return Ok(false),
            key_event = events_stream.next() => {
                match key_event {
                    Some(Ok(crossterm::event::Event::Key(key)))
                        if key.kind == crossterm::event::KeyEventKind::Press
                            && key.code == crossterm::event::KeyCode::Esc => return Ok(true),
                    Some(_) => {}
                    None => return Ok(false),
                }
            }
        }
    }
}
