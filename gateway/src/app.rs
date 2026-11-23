use axum::{
    Router,
    routing::{get, post},
};

use crate::{
    internal::routes::{archive_stats, close_room, list_rooms, store_stats},
    state::AppState,
    ws::handler::ws_upgrade,
};

/// 클라이언트가 붙는 공개 라우터.
pub fn public_router(state: AppState) -> Router {
    Router::new()
        .route("/ws", get(ws_upgrade))
        .route("/healthz", get(|| async { "ok" }))
        .with_state(state)
}

/// 관리 API만 접근하는 내부 라우터.
pub fn internal_router(state: AppState) -> Router {
    Router::new()
        .route("/internal/rooms", get(list_rooms))
        .route("/internal/store", get(store_stats))
        .route("/internal/archive", get(archive_stats))
        .route("/internal/rooms/{room_id}/close", post(close_room))
        .with_state(state)
}
