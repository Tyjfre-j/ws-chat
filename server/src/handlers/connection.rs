use axum::{
    extract::State,
    extract::ws::{Message, WebSocket, WebSocketUpgrade},
    response::IntoResponse,
};

use futures_util::{
    StreamExt,
    stream::{SplitSink, SplitStream},
};

use crate::net::send_server_message;
use crate::protocol::ServerMessage;
use crate::state::AppState;

use super::{
    MAX_MESSAGE_SIZE, chat::run_chat_loop, room::select_room, username::get_confirmed_username,
};

pub(super) type WsWrite = SplitSink<WebSocket, Message>;
pub(super) type WsRead = SplitStream<WebSocket>;

pub async fn handle_health() -> &'static str {
    "OK"
}

pub async fn ws_handler(State(state): State<AppState>, ws: WebSocketUpgrade) -> impl IntoResponse {
    ws.max_message_size(MAX_MESSAGE_SIZE)
        .on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(socket: WebSocket, state: AppState) {
    let (mut write, mut read) = socket.split();

    if !send_server_message(&mut write, &ServerMessage::Welcome).await {
        return;
    }

    let Some(username) = get_confirmed_username(&mut write, &mut read, &state).await else {
        return;
    };

    let Some(room) = select_room(&mut write, &mut read, &state).await else {
        state.usernames.remove(&username.to_lowercase());
        return;
    };

    run_chat_loop(&mut write, &mut read, &state, username.clone(), room).await;

    state.usernames.remove(&username.to_lowercase());
}
