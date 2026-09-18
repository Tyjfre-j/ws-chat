use crossterm::event::EventStream;
use futures_util::StreamExt;
use ratatui::DefaultTerminal;
use tokio_tungstenite::connect_async;

use crate::app::App;
use crate::events::{self, KeyOutcome};
use crate::net;
use crate::protocol::ClientStage;

pub async fn run(terminal: &mut DefaultTerminal, app: &mut App) -> std::io::Result<()> {
    let (ws_stream, _response) = connect_async("ws://127.0.0.1:3000/ws")
        .await
        .expect("failed to connect");

    let (mut write, mut read) = ws_stream.split();
    let mut events_stream = EventStream::new();

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
                            return Ok(());
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
    app.messages.push("Disconnected from server".to_string());
    app.stage = ClientStage::Disconnected;

    Ok(())
}
