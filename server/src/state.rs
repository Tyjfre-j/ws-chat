use std::sync::Arc;
use dashmap::DashMap;
use tokio::sync::broadcast;

use crate::protocol::ServerMessage;

#[derive(Clone)]
pub struct AppState {
    pub rooms: Arc<DashMap<String, broadcast::Sender<ServerMessage>>>,
}