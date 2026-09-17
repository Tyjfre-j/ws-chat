use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::protocol::ServerMessage;

#[derive(Clone)]
pub struct AppState {
    pub rooms: Arc<DashMap<String, broadcast::Sender<ServerMessage>>>,
}
