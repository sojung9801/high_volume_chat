use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};

use serde::Deserialize;

use crate::{archive::ArchiveStats, room::room::RoomStats, state::AppState, store::StoreStats};

/// 관리 API가 방 현황을 조회하는 내부 엔드포인트.
pub async fn list_rooms(State(state): State<AppState>) -> Json<Vec<RoomStats>> {
    Json(state.rooms.stats())
}

/// 저장소 상태. 저장하지 못한 메시지 수를 본다.
pub async fn store_stats(State(state): State<AppState>) -> Json<StoreStats> {
    Json(state.store.stats())
}

#[derive(Deserialize)]
pub struct CloseRoomQuery {
    /// 이 회차까지의 방을 닫는다. 같은 ID로 다시 연 더 새로운 방은 건드리지 않는다.
    generation: u64,
}

/// 방을 닫고 붙어 있는 연결을 모두 내보낸다. 관리 API가 방장 확인을 마친 뒤 부른다.
/// 그런 방이 떠 있지 않아도 성공으로 본다 (이미 아무도 없다는 뜻이다).
pub async fn close_room(
    State(state): State<AppState>,
    Path(room_id): Path<String>,
    Query(query): Query<CloseRoomQuery>,
) -> StatusCode {
    state.rooms.close(&room_id, query.generation);
    StatusCode::NO_CONTENT
}

/// 보관 상태. 쓴 파일 수와 보관하지 못한 메시지 수를 본다.
pub async fn archive_stats(State(state): State<AppState>) -> Json<ArchiveStats> {
    Json(state.archive.stats())
}
