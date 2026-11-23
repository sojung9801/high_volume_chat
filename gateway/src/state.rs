use std::sync::Arc;

use crate::{
    archive::MessageArchive, auth::JoinTokenVerifier, config::SessionConfig, room::registry::RoomRegistry,
    store::MessageStore,
};

#[derive(Clone)]
pub struct AppState {
    pub rooms: Arc<RoomRegistry>,
    pub verifier: Arc<JoinTokenVerifier>,
    pub session: SessionConfig,
    pub store: MessageStore,
    pub archive: MessageArchive,
}
