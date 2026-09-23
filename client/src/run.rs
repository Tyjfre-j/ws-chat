use crossterm::event::EventStream;
use futures_util::{SinkExt, StreamExt};
use ratatui::DefaultTerminal;
use tokio_tungstenite::connect_async;

use crate::app::{App, ChatEvent, handle_server_message};
use crate::events::{self, KeyOutcome};
use crate::net;
use crate::protocol::{ClientStage, Received, RetryOutcome, RunOutcome};

const DEFAULT_SERVER_URL: &str = "ws://127.0.0.1:3000/ws";
const CONNECT_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(8);

pub enum ExitReason {
    UserQuit,
    InputFailed,
}

pub async fn run_connection(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    events_stream: &mut EventStream,
) -> std::io::Result<RunOutcome> {
    app.connected = false;
    app.input.clear();
    app.stage = ClientStage::Connecting;
    terminal.draw(|frame| crate::ui::render(frame, app))?;

    let server_url =
        std::env::var("WS_CHAT_SERVER_URL").unwrap_or_else(|_| DEFAULT_SERVER_URL.to_string());

    let ws_stream = match tokio::time::timeout(CONNECT_TIMEOUT, connect_async(&server_url)).await {
        Ok(Ok((ws_stream, _response))) => ws_stream,
        Ok(Err(e)) => {
            tracing::warn!(error = %e, server_url = %server_url, "failed to connect to server");
            app.push_event(ChatEvent::System("Couldn't reach the server.".to_string()));
            app.stage = ClientStage::Disconnected;
            return Ok(RunOutcome::ConnectionFailed);
        }
        Err(_) => {
            tracing::warn!(server_url = %server_url, "connection attempt timed out");
            app.push_event(ChatEvent::System(
                "Connection attempt timed out.".to_string(),
            ));
            app.stage = ClientStage::Disconnected;
            return Ok(RunOutcome::ConnectionFailed);
        }
    };

    let (mut write, mut read) = ws_stream.split();

    loop {
        terminal.draw(|frame| crate::ui::render(frame, app))?;

        tokio::select! {
            server_msg = read.next() => {
                match net::receive_server_message(server_msg) {
                    Received::Message(msg) => {
                        handle_server_message(app, msg);
                    }
                    Received::Ignored => {}
                    Received::Unsupported | Received::Invalid => {
                        app.push_event(ChatEvent::System(
                            "Received an unexpected message from the server. Connection lost."
                                .to_string(),
                        ));
                        break;
                    }
                    Received::Disconnected => {
                        app.push_event(ChatEvent::System(
                            "Server closed the connection.".to_string(),
                        ));
                        break;
                    }
                }
            }

            key_event = events_stream.next() => {
                match key_event {
                    Some(Ok(crossterm::event::Event::Key(key)))
                        if key.kind == crossterm::event::KeyEventKind::Press =>
                    {
                        match events::handle_key(app, &mut write, key).await {
                            KeyOutcome::Quit => {
                                if let Err(error) = write.close().await {
                                    tracing::debug!(%error, "failed to close WebSocket cleanly");
                                }
                                return Ok(RunOutcome::Quit);
                            }
                            KeyOutcome::Disconnected => break,
                            KeyOutcome::Continue => {}
                        }
                    }

                    Some(Ok(_)) => {}

                    Some(Err(e)) => {
                        tracing::error!(error = ?e, "keyboard event stream failed");
                        app.push_event(ChatEvent::System(
                            "The client can no longer receive keyboard input.".to_string(),
                        ));
                        return Ok(RunOutcome::InputFailed);
                    }

                    None => {
                        tracing::error!("keyboard event stream ended");
                        app.push_event(ChatEvent::System(
                            "The client can no longer receive keyboard input.".to_string(),
                        ));
                        return Ok(RunOutcome::InputFailed);
                    }
                }
            }
        }
    }

    app.connected = false;
    app.stage = ClientStage::Disconnected;

    Ok(RunOutcome::Disconnected)
}

pub async fn run(terminal: &mut DefaultTerminal, app: &mut App) -> std::io::Result<ExitReason> {
    let mut events_stream = EventStream::new();
    let mut backoff = std::time::Duration::from_secs(1);

    loop {
        match run_connection(terminal, app, &mut events_stream).await? {
            RunOutcome::Quit => return Ok(ExitReason::UserQuit),

            RunOutcome::InputFailed => {
                tracing::error!("client input system failed");
                return Ok(ExitReason::InputFailed);
            }

            RunOutcome::Disconnected => {
                backoff = std::time::Duration::from_secs(1);

                app.push_event(ChatEvent::System(format!(
                    "Retrying in {}s...",
                    backoff.as_secs()
                )));

                match wait_before_retry(terminal, app, &mut events_stream, backoff).await? {
                    RetryOutcome::Quit => return Ok(ExitReason::UserQuit),
                    RetryOutcome::InputFailed => return Ok(ExitReason::InputFailed),
                    RetryOutcome::Retry => {}
                }

                backoff = (backoff * 2).min(std::time::Duration::from_secs(30));
            }

            RunOutcome::ConnectionFailed => {
                app.push_event(ChatEvent::System(format!(
                    "Retrying in {}s...",
                    backoff.as_secs()
                )));

                match wait_before_retry(terminal, app, &mut events_stream, backoff).await? {
                    RetryOutcome::Quit => return Ok(ExitReason::UserQuit),
                    RetryOutcome::InputFailed => return Ok(ExitReason::InputFailed),
                    RetryOutcome::Retry => {}
                }

                backoff = (backoff * 2).min(std::time::Duration::from_secs(30));
            }
        }
    }
}

async fn wait_before_retry(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    events_stream: &mut EventStream,
    backoff: std::time::Duration,
) -> std::io::Result<RetryOutcome> {
    let retry_timer = tokio::time::sleep(backoff);
    tokio::pin!(retry_timer);

    loop {
        terminal.draw(|frame| crate::ui::render(frame, app))?;

        tokio::select! {
            _ = &mut retry_timer => return Ok(RetryOutcome::Retry),

            key_event = events_stream.next() => {
                match key_event {
                    Some(Ok(crossterm::event::Event::Key(key)))
                        if key.kind == crossterm::event::KeyEventKind::Press =>
                    {
                        if let KeyOutcome::Quit = events::handle_retry_wait_key(app, key) {
                            return Ok(RetryOutcome::Quit);
                        }
                    }

                    Some(Ok(_)) => {}

                    Some(Err(e)) => {
                        tracing::error!(error = ?e, "keyboard event stream failed");
                        app.push_event(ChatEvent::System(
                            "The client can no longer receive keyboard input.".to_string(),
                        ));
                        return Ok(RetryOutcome::InputFailed);
                    }

                    None => {
                        tracing::error!("keyboard event stream ended");
                        app.push_event(ChatEvent::System(
                            "The client can no longer receive keyboard input.".to_string(),
                        ));
                        return Ok(RetryOutcome::InputFailed);
                    }
                }
            }
        }
    }
}
