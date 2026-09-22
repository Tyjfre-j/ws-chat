use crossterm::event::EventStream;
use futures_util::{SinkExt, StreamExt};
use ratatui::DefaultTerminal;
use tokio_tungstenite::connect_async;

use crate::app::{App, ChatEvent};
use crate::events::{self, KeyOutcome};
use crate::net;
use crate::protocol::ClientStage;
use crate::protocol::RunOutcome;

const DEFAULT_SERVER_URL: &str = "ws://127.0.0.1:3000/ws";

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

    let ws_stream = match connect_async(&server_url).await {
        Ok((ws_stream, _response)) => ws_stream,
        Err(e) => {
            tracing::warn!(error = %e, server_url = %server_url, "failed to connect to server");
            app.connected = false;
            app.push_event(ChatEvent::System("Couldn't reach the server.".to_string()));
            app.stage = ClientStage::Disconnected;
            return Ok(RunOutcome::ConnectionFailed);
        }
    };

    let (mut write, mut read) = ws_stream.split();

    loop {
        terminal.draw(|frame| crate::ui::render(frame, app))?;

        tokio::select! {
            server_msg = read.next() => {
                let server_msg = match server_msg {
                    Some(Ok(tokio_tungstenite::tungstenite::Message::Ping(payload))) => {
                        if let Err(error) = write
                            .send(tokio_tungstenite::tungstenite::Message::Pong(payload))
                            .await
                        {
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
                        if key.kind == crossterm::event::KeyEventKind::Press =>
                    {
                        match events::handle_key(app, &mut write, key).await {
                            KeyOutcome::Quit => {
                                let _ = write.close().await;
                                return Ok(RunOutcome::Quit);
                            }
                            KeyOutcome::Disconnected => break,
                            KeyOutcome::Continue => {}
                        }
                    }

                    Some(Ok(_)) => {}

                    Some(Err(e)) => {
                        tracing::error!(error = ?e, "keyboard event stream failed");
                        return Ok(RunOutcome::InputFailed);
                    }

                    None => {
                        tracing::error!("keyboard event stream ended");
                        return Ok(RunOutcome::InputFailed);
                    }
                }
            }
        }
    }

    app.connected = false;
    app.push_event(ChatEvent::System("Disconnected from server".to_string()));
    app.stage = ClientStage::Disconnected;

    Ok(RunOutcome::Disconnected)
}

pub async fn run(terminal: &mut DefaultTerminal, app: &mut App) -> std::io::Result<()> {
    let mut events_stream = EventStream::new();
    let mut backoff = std::time::Duration::from_secs(1);

    loop {
        match run_connection(terminal, app, &mut events_stream).await? {
            RunOutcome::Quit => break,

            RunOutcome::InputFailed => {
                tracing::error!("client input system failed");
                break;
            }

            RunOutcome::Disconnected => {
                backoff = std::time::Duration::from_secs(1);

                app.push_event(ChatEvent::System(format!(
                    "Reconnecting in {}s...",
                    backoff.as_secs()
                )));

                match wait_before_retry(terminal, app, &mut events_stream, backoff).await? {
                    RunOutcome::Quit | RunOutcome::InputFailed => break,
                    RunOutcome::Disconnected => {}
                    RunOutcome::ConnectionFailed => {}
                }

                backoff = (backoff * 2).min(std::time::Duration::from_secs(30));
            }

            RunOutcome::ConnectionFailed => {
                app.push_event(ChatEvent::System(format!(
                    "Retrying in {}s...",
                    backoff.as_secs()
                )));

                match wait_before_retry(terminal, app, &mut events_stream, backoff).await? {
                    RunOutcome::Quit | RunOutcome::InputFailed => break,
                    RunOutcome::Disconnected => {}
                    RunOutcome::ConnectionFailed => {}
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
) -> std::io::Result<RunOutcome> {
    let retry_timer = tokio::time::sleep(backoff);
    tokio::pin!(retry_timer);

    loop {
        terminal.draw(|frame| crate::ui::render(frame, app))?;

        tokio::select! {
            _ = &mut retry_timer => return Ok(RunOutcome::Disconnected),

            key_event = events_stream.next() => {
                match key_event {
                    Some(Ok(crossterm::event::Event::Key(key)))
                        if key.kind == crossterm::event::KeyEventKind::Press =>
                    {
                        if key.code != crossterm::event::KeyCode::Enter
                            && let KeyOutcome::Quit = events::handle_local_key(app, key)
                        {
                            return Ok(RunOutcome::Quit);
                        }
                    }

                    Some(Ok(_)) => {}

                    Some(Err(e)) => {
                        tracing::error!(error = ?e, "keyboard event stream failed");
                        return Ok(RunOutcome::InputFailed);
                    }

                    None => {
                        tracing::error!("keyboard event stream ended");
                        return Ok(RunOutcome::InputFailed);
                    }
                }
            }
        }
    }
}
