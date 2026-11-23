//! 방장 표시와 방 닫기.

mod support;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chat_gateway::{
    config::{RoomConfig, SessionConfig},
    proto::chat::v1::server_frame::Payload,
};
use futures_util::StreamExt;
use sqlx::{PgPool, Row};

use support::{
    ClientSocket, TestGateway, collect_until_quiet, decode_server_frame, send_text,
    test_database_url, wait_until,
};

const QUIET_PERIOD: Duration = Duration::from_millis(400);
const WAIT_TIMEOUT: Duration = Duration::from_secs(5);

fn room_config() -> RoomConfig {
    RoomConfig {
        flush_interval: Duration::from_millis(50),
        max_messages_per_flush: 4,
        broadcast_capacity: 64,
    }
}

fn session_config() -> SessionConfig {
    SessionConfig {
        send_timeout: Duration::from_secs(5),
        rate_limit_burst: 100,
        rate_limit_per_sec: 100.0,
        max_text_chars: 300,
        max_frame_bytes: 4096,
    }
}

fn unique_room_id(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_nanos();
    format!("{prefix}-{nanos}")
}

/// 연결이 끝날 때까지 읽어서 받은 프레임 종류를 돌려준다.
async fn read_until_disconnected(socket: &mut ClientSocket) -> Vec<Payload> {
    let mut payloads = Vec::new();
    loop {
        match tokio::time::timeout(WAIT_TIMEOUT, socket.next()).await {
            Ok(Some(Ok(message))) => payloads.extend(decode_server_frame(&message)),
            Ok(Some(Err(_))) | Ok(None) => return payloads,
            Err(_) => panic!("connection should have been closed by the gateway"),
        }
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn owner_messages_are_flagged_and_stored_as_such() {
    let pool = PgPool::connect(&test_database_url())
        .await
        .expect("test database should be reachable (docker compose up -d)");
    let gateway = TestGateway::start(room_config(), session_config()).await;
    let room_id = unique_room_id("owner-room");
    let mut owner = gateway.connect_as_owner(&room_id, "host").await;
    let mut member = gateway.connect(&room_id, "guest").await;

    send_text(&mut owner, "방장입니다").await;
    tokio::time::sleep(Duration::from_millis(80)).await;
    send_text(&mut member, "손님입니다").await;

    let flags: Vec<(String, bool)> = collect_until_quiet(&mut member, QUIET_PERIOD)
        .await
        .into_iter()
        .filter_map(|payload| match payload {
            Payload::Batch(batch) => Some(batch.messages),
            _ => None,
        })
        .flatten()
        .map(|message| (message.nickname, message.is_owner))
        .collect();
    assert_eq!(flags, [("host".to_owned(), true), ("guest".to_owned(), false)]);

    let deadline = tokio::time::Instant::now() + WAIT_TIMEOUT;
    loop {
        let stored: Vec<(String, bool)> = sqlx::query(
            "SELECT nickname, is_owner FROM chat_messages WHERE room_id = $1 ORDER BY sent_at_ms, seq",
        )
        .bind(&room_id)
        .fetch_all(&pool)
        .await
        .expect("stored rows should load")
        .into_iter()
        .map(|row| (row.get(0), row.get(1)))
        .collect();
        if stored == flags {
            break;
        }
        assert!(tokio::time::Instant::now() < deadline, "stored rows were {stored:?}");
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

/// 방을 닫으면 모두에게 알리고 연결을 끊는다. 방은 레지스트리에서 사라진다.
#[tokio::test(flavor = "multi_thread")]
async fn closing_a_room_notifies_and_disconnects_everyone() {
    let gateway = TestGateway::start(room_config(), session_config()).await;
    let room_id = unique_room_id("closing-room");
    let mut owner = gateway.connect_as_owner(&room_id, "host").await;
    let mut member = gateway.connect(&room_id, "guest").await;
    wait_until("both joined", WAIT_TIMEOUT, || {
        gateway.room_stats(&room_id).is_some_and(|stats| stats.connections == 2)
    })
    .await;

    assert!(gateway.close_room(&room_id));

    for socket in [&mut owner, &mut member] {
        let payloads = read_until_disconnected(socket).await;
        assert!(
            matches!(payloads.last(), Some(Payload::RoomClosed(_))),
            "last frame before disconnect should be RoomClosed, got {payloads:?}"
        );
    }
    wait_until("closed room removed", WAIT_TIMEOUT, || gateway.room_count() == 0).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn closing_an_unknown_room_reports_nothing_to_close() {
    let gateway = TestGateway::start(room_config(), session_config()).await;

    assert!(!gateway.close_room("never-opened"));
}
