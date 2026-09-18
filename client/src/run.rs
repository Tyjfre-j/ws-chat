use crossterm::event::EventStream;
use futures_util::StreamExt;
use ratatui::DefaultTerminal;
use tokio_tungstenite::connect_async;

use crate::app::App;
use crate::events::{self, KeyOutcome};
use crate::net;
use crate::protocol::ClientStage;
use crate::protocol::ConnectionOutcome;

pub async fn run_connection(
    terminal: &mut DefaultTerminal,
    app: &mut App,
    events_stream: &mut EventStream,
) -> std::io::Result<ConnectionOutcome> {
    let ws_stream = match connect_async("ws://127.0.0.1:3000/ws").await {
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
                if !net::handle_incoming(app, server_msg) {
                    break;
                }
            }
            key_event = events_stream.next() => {
                match key_event {
                    Some(Ok(crossterm::event::Event::Key(key))) => {
                        if let KeyOutcome::Quit = events::handle_key(app, &mut write, key).await {
                            return Ok(ConnectionOutcome::Quit);
                        }
                    }
                    Some(Ok(_)) => {}
                    Some(Err(e)) => eprintln!("error reading key event: {e:?}"),
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
    // returns Ok(true) if the user pressed Esc (caller should quit)
    terminal.draw(|frame| crate::ui::render(frame, app))?;

    tokio::select! {
        _ = tokio::time::sleep(backoff) => {}
        key_event = events_stream.next() => {
            if let Some(Ok(crossterm::event::Event::Key(key))) = key_event {
                if key.code == crossterm::event::KeyCode::Esc {
                    return Ok(true);
                }
            }
        }
    }

    Ok(false)
}
