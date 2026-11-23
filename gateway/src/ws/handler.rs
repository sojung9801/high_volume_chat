use axum::{
    extract::{Query, State, ws::WebSocketUpgrade},
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde::Deserialize;

use crate::state::AppState;

use super::session::run_session;

#[derive(Deserialize)]
pub struct JoinQuery {
    /// 브라우저 WebSocket은 헤더를 못 실으므로 쿼리로 받는다. 토큰 수명은 짧게 발급된다.
    token: String,
}

pub async fn ws_upgrade(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    Query(query): Query<JoinQuery>,
) -> Response {
    match state.verifier.verify(&query.token) {
        Ok(claims) => ws
            .max_message_size(state.session.max_frame_bytes)
            .on_upgrade(move |socket| run_session(socket, state, claims)),
        Err(err) => {
            tracing::debug!(%err, "join token rejected");
            StatusCode::UNAUTHORIZED.into_response()
        }
    }
}
