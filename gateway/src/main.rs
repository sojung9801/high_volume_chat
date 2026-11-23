use std::sync::Arc;

use anyhow::Result;
use chat_gateway::{
    archive::MessageArchive,
    app::{internal_router, public_router},
    auth::JoinTokenVerifier,
    config::Config,
    room::registry::RoomRegistry,
    state::AppState,
    store::MessageStore,
};
use tokio::net::TcpListener;

#[tokio::main]
async fn main() -> Result<()> {
    // 로컬 개발용. 운영에서는 환경 변수를 직접 주입한다.
    dotenvy::dotenv().ok();
    tracing_subscriber::fmt::init();

    let config = Config::from_env()?;
    let store = MessageStore::start(&config.store)?;
    let archive = MessageArchive::start(&config.archive)?;
    let state = AppState {
        rooms: Arc::new(RoomRegistry::new(config.room, store.clone(), archive.clone())),
        verifier: Arc::new(JoinTokenVerifier::new(&config.join_token_secret)?),
        session: config.session,
        store,
        archive,
    };

    let public = public_router(state.clone());
    let internal = internal_router(state);

    let public_listener = TcpListener::bind(config.public_addr).await?;
    let internal_listener = TcpListener::bind(config.internal_addr).await?;
    tracing::info!(public = %config.public_addr, internal = %config.internal_addr, "gateway listening");

    tokio::try_join!(
        async { axum::serve(public_listener, public).await },
        async { axum::serve(internal_listener, internal).await },
    )?;

    Ok(())
}
