use std::time::Duration;

use axum::extract::ws::{Message, WebSocket};
use bytes::Bytes;
use futures_util::{SinkExt, StreamExt, stream::SplitSink};
use prost::Message as _;
use tokio::sync::{broadcast::error::RecvError, watch};

use crate::{
    auth::JoinClaims,
    config::SessionConfig,
    proto::chat::v1::{
        ClientFrame, ErrorCode, RoomClosed, ServerError, ServerFrame, client_frame, server_frame,
    },
    rate_limit::TokenBucket,
    room::room::{Room, Sender},
    state::AppState,
};

/// 연결 하나의 수명. 방 프레임을 내려보내고 클라이언트 프레임을 받는다.
pub async fn run_session(socket: WebSocket, state: AppState, claims: JoinClaims) {
    let config = state.session;
    let mut membership = state.rooms.join(&claims.room);
    let room = membership.room();
    let (mut sink, mut stream) = socket.split();
    let mut limiter = TokenBucket::new(config.rate_limit_burst, config.rate_limit_per_sec);
    let mut closed = room.closed_signal();

    loop {
        tokio::select! {
            // 방장이 방을 닫았다. 이유를 알려주고 끊는다. (이미 닫힌 방에 들어온 경우도 바로 여기로 온다.)
            () = wait_until_closed(&mut closed) => {
                send_frame(&mut sink, encode_room_closed(), config.send_timeout).await;
                break;
            }
            outbound = membership.recv() => match outbound {
                Ok(frame) => {
                    if !send_frame(&mut sink, frame, config.send_timeout).await {
                        break;
                    }
                }
                // 이 연결이 느려서 버퍼가 넘쳤다. 놓친 프레임은 버리고 최신부터 이어간다.
                Err(RecvError::Lagged(skipped)) => room.record_lag(skipped),
                Err(RecvError::Closed) => break,
            },
            inbound = stream.next() => match inbound {
                Some(Ok(Message::Binary(data))) => {
                    if let Some(code) = handle_client_frame(data, &room, &claims, &mut limiter, config) {
                        if !send_frame(&mut sink, encode_error(code), config.send_timeout).await {
                            break;
                        }
                    }
                }
                Some(Ok(Message::Close(_))) | Some(Err(_)) | None => break,
                // ping/pong은 axum이 처리한다. 텍스트 프레임은 프로토콜에 없다.
                Some(Ok(_)) => {}
            },
        }
    }
}

fn handle_client_frame(
    data: Bytes,
    room: &Room,
    claims: &JoinClaims,
    limiter: &mut TokenBucket,
    config: SessionConfig,
) -> Option<ErrorCode> {
    let Ok(frame) = ClientFrame::decode(data) else {
        return Some(ErrorCode::InvalidMessage);
    };

    match frame.payload {
        Some(client_frame::Payload::SendMessage(message)) => {
            if !limiter.try_acquire() {
                return Some(ErrorCode::RateLimited);
            }

            let text = message.text.trim();
            if text.is_empty() || text.chars().count() > config.max_text_chars {
                return Some(ErrorCode::InvalidMessage);
            }

            // 샘플링으로 버려진 경우는 오류가 아니다. 방이 붐빌 때의 정상 동작이다.
            let sender = Sender {
                user_id: &claims.sub,
                nickname: &claims.nick,
                is_owner: claims.owner,
            };
            room.publish(sender, text.to_owned());
            None
        }
        None => Some(ErrorCode::InvalidMessage),
    }
}

fn encode_error(code: ErrorCode) -> Bytes {
    let frame = ServerFrame {
        payload: Some(server_frame::Payload::Error(ServerError { code: code.into() })),
    };
    Bytes::from(frame.encode_to_vec())
}

/// 방이 닫힐 때까지 기다린다.
async fn wait_until_closed(closed: &mut watch::Receiver<bool>) {
    // 신호를 보내는 쪽(방)은 이 연결이 붙어 있는 동안 살아 있다. 그래도 사라졌다면
    // "닫힘"으로 오해하지 않도록 영원히 기다린다.
    if closed.wait_for(|is_closed| *is_closed).await.is_err() {
        std::future::pending::<()>().await;
    }
}

fn encode_room_closed() -> Bytes {
    let frame = ServerFrame {
        payload: Some(server_frame::Payload::RoomClosed(RoomClosed {})),
    };
    Bytes::from(frame.encode_to_vec())
}

/// 제한 시간 안에 못 보내면 죽은 연결로 보고 `false`를 돌려준다.
async fn send_frame(
    sink: &mut SplitSink<WebSocket, Message>,
    frame: Bytes,
    timeout: Duration,
) -> bool {
    matches!(
        tokio::time::timeout(timeout, sink.send(Message::Binary(frame))).await,
        Ok(Ok(()))
    )
}
