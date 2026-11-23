//! 통합 테스트 공용 도구. 실제 게이트웨이를 임의 포트에 띄우고 실제 WebSocket으로 붙는다.
#![allow(dead_code)]

use std::{
    net::SocketAddr,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use chat_gateway::{
    app::public_router,
    archive::{ArchiveStats, MessageArchive},
    auth::JoinTokenVerifier,
    config::{ArchiveConfig, RoomConfig, SessionConfig, StoreConfig},
    proto::chat::v1::{ClientFrame, SendMessage, ServerFrame, client_frame, server_frame},
    room::{registry::RoomRegistry, room::RoomStats},
    state::AppState,
    store::{MessageStore, StoreStats},
};
use futures_util::{SinkExt, StreamExt};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use prost::Message as _;
use serde::Serialize;
use tokio::net::{TcpListener, TcpStream};
use tokio_tungstenite::{
    MaybeTlsStream, WebSocketStream, connect_async,
    tungstenite::{self, Message},
};

const JOIN_TOKEN_SECRET: &str = "integration-test-join-token-secret-0123456789";
const JOIN_TOKEN_AUDIENCE: &str = "chat-gateway";
const JOIN_TOKEN_TTL_SEC: u64 = 60;
const POLL_INTERVAL: Duration = Duration::from_millis(10);

pub type ClientSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;

#[derive(Serialize)]
struct JoinTokenClaims<'a> {
    sub: &'a str,
    room: &'a str,
    nick: &'a str,
    owner: bool,
    aud: &'a str,
    exp: u64,
}

/// 테스트 안에서 돌아가는 게이트웨이. 테스트 런타임이 끝나면 같이 내려간다.
pub struct TestGateway {
    addr: SocketAddr,
    rooms: Arc<RoomRegistry>,
    store: MessageStore,
    archive: MessageArchive,
    archive_dir: PathBuf,
}

/// 테스트에서는 파일이 빨리 닫히도록 주기를 짧게 둔다.
const ARCHIVE_FLUSH_INTERVAL: Duration = Duration::from_millis(200);

/// 게이트웨이마다 다른 임시 폴더. 테스트끼리 파일이 섞이지 않는다.
fn unique_archive_dir() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("chat-archive-test-{}-{nanos}", std::process::id()))
}

/// 테스트용 DB 주소. 저장소 루트 `.env`의 `TEST_DATABASE_URL`을 쓴다.
/// `docker compose up -d`로 Postgres가 떠 있어야 한다.
pub fn test_database_url() -> String {
    dotenvy::dotenv().ok();
    std::env::var("TEST_DATABASE_URL").expect("TEST_DATABASE_URL is required for tests")
}

pub fn store_config(database_url: String) -> StoreConfig {
    StoreConfig {
        database_url,
        queue_capacity: 10_000,
        batch_max_messages: 500,
        max_connections: 4,
    }
}

/// 테스트가 끝나면 임시 보관 폴더를 지운다.
impl Drop for TestGateway {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.archive_dir);
    }
}

impl TestGateway {
    pub async fn start(room: RoomConfig, session: SessionConfig) -> Self {
        Self::start_with_store(room, session, store_config(test_database_url())).await
    }

    pub async fn start_with_store(
        room: RoomConfig,
        session: SessionConfig,
        store: StoreConfig,
    ) -> Self {
        let store = MessageStore::start(&store).expect("message store should start");
        let archive_dir = unique_archive_dir();
        let archive = MessageArchive::start(&ArchiveConfig {
            dir: archive_dir.clone(),
            flush_interval: ARCHIVE_FLUSH_INTERVAL,
            max_messages_per_file: 100_000,
            queue_capacity: 1_000_000,
        })
        .expect("message archive should start");
        let rooms = Arc::new(RoomRegistry::new(room, store.clone(), archive.clone()));
        let state = AppState {
            rooms: Arc::clone(&rooms),
            verifier: Arc::new(
                JoinTokenVerifier::new(JOIN_TOKEN_SECRET).expect("verifier should build"),
            ),
            session,
            store: store.clone(),
            archive: archive.clone(),
        };

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("test listener should bind");
        let addr = listener.local_addr().expect("listener should have an address");
        tokio::spawn(async move {
            axum::serve(listener, public_router(state))
                .await
                .expect("test gateway should keep serving");
        });

        Self {
            addr,
            rooms,
            store,
            archive,
            archive_dir,
        }
    }

    /// 관리 API가 하는 것과 같은 방식으로 입장 토큰을 서명한다.
    pub fn join_token(&self, room_id: &str, user_id: &str, nickname: &str) -> String {
        self.join_token_as(room_id, user_id, nickname, false)
    }

    pub fn join_token_as(
        &self,
        room_id: &str,
        user_id: &str,
        nickname: &str,
        is_owner: bool,
    ) -> String {
        let exp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_secs()
            + JOIN_TOKEN_TTL_SEC;
        let claims = JoinTokenClaims {
            sub: user_id,
            room: room_id,
            nick: nickname,
            owner: is_owner,
            aud: JOIN_TOKEN_AUDIENCE,
            exp,
        };
        let key = EncodingKey::from_secret(JOIN_TOKEN_SECRET.as_bytes())
            .expect("encoding key should build");

        encode(&Header::new(Algorithm::HS256), &claims, &key).expect("token should sign")
    }

    pub async fn connect_with_token(&self, token: &str) -> Result<ClientSocket, tungstenite::Error> {
        let url = format!("ws://{}/ws?token={token}", self.addr);
        connect_async(url).await.map(|(socket, _)| socket)
    }

    pub async fn connect(&self, room_id: &str, user_id: &str) -> ClientSocket {
        let token = self.join_token(room_id, user_id, user_id);
        self.connect_with_token(&token)
            .await
            .expect("client should connect")
    }

    pub async fn connect_as_owner(&self, room_id: &str, user_id: &str) -> ClientSocket {
        let token = self.join_token_as(room_id, user_id, user_id, true);
        self.connect_with_token(&token)
            .await
            .expect("owner should connect")
    }

    /// 관리 API가 내부 포트로 요청했을 때와 같은 동작.
    pub fn close_room(&self, room_id: &str) -> bool {
        self.rooms.close(room_id)
    }

    pub fn room_stats(&self, room_id: &str) -> Option<RoomStats> {
        self.rooms
            .stats()
            .into_iter()
            .find(|stats| stats.room_id == room_id)
    }

    pub fn archive_stats(&self) -> ArchiveStats {
        self.archive.stats()
    }

    pub fn archive_dir(&self) -> &Path {
        &self.archive_dir
    }

    pub fn store_stats(&self) -> StoreStats {
        self.store.stats()
    }

    pub fn room_count(&self) -> usize {
        self.rooms.stats().len()
    }
}

pub fn encode_send_message(text: &str) -> Message {
    let frame = ClientFrame {
        payload: Some(client_frame::Payload::SendMessage(SendMessage {
            text: text.to_owned(),
        })),
    };
    Message::Binary(frame.encode_to_vec().into())
}

pub async fn send_text(socket: &mut ClientSocket, text: &str) {
    socket
        .send(encode_send_message(text))
        .await
        .expect("client frame should send");
}

/// 바이너리 프레임만 풀어서 돌려준다. 그 외(ping 등)는 `None`.
pub fn decode_server_frame(message: &Message) -> Option<server_frame::Payload> {
    let Message::Binary(data) = message else {
        return None;
    };
    ServerFrame::decode(data.as_ref())
        .expect("server frame should decode")
        .payload
}

/// `quiet` 동안 아무것도 안 올 때까지 서버 프레임을 모은다.
pub async fn collect_until_quiet(
    socket: &mut ClientSocket,
    quiet: Duration,
) -> Vec<server_frame::Payload> {
    let mut payloads = Vec::new();
    while let Ok(Some(Ok(message))) = tokio::time::timeout(quiet, socket.next()).await {
        if let Some(payload) = decode_server_frame(&message) {
            payloads.push(payload);
        }
    }
    payloads
}

/// 조건이 참이 될 때까지 기다린다. 제한 시간을 넘기면 테스트를 실패시킨다.
pub async fn wait_until(label: &str, timeout: Duration, mut condition: impl FnMut() -> bool) {
    let deadline = tokio::time::Instant::now() + timeout;
    while !condition() {
        assert!(
            tokio::time::Instant::now() < deadline,
            "timed out waiting for: {label}"
        );
        tokio::time::sleep(POLL_INTERVAL).await;
    }
}
