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

    assert!(gateway.close_room(&room_id, 0));

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

    assert!(!gateway.close_room("never-opened", 0));
}

/// 방이 닫힌 뒤에도 입장 토큰은 60초 동안 서명이 유효하다.
/// 모두 나가서 방이 사라진 뒤에 그 토큰으로 들어와도 방이 되살아나면 안 된다.
#[tokio::test(flavor = "multi_thread")]
async fn closed_room_cannot_be_revived_with_a_token_issued_before_closing() {
    const GENERATION: u64 = 1_795_000_000_000;
    let gateway = TestGateway::start(room_config(), session_config()).await;
    let room_id = unique_room_id("revive-room");
    let mut member = gateway.connect_in_generation(&room_id, "guest", GENERATION).await;
    // 닫히기 전에 받아 둔 토큰.
    let old_token = gateway.join_token_with(&room_id, "late", "late", true, Some(GENERATION));

    assert!(gateway.close_room(&room_id, GENERATION));
    read_until_disconnected(&mut member).await;
    wait_until("closed room removed", WAIT_TIMEOUT, || gateway.room_count() == 0).await;

    let mut late = gateway
        .connect_with_token(&old_token)
        .await
        .expect("handshake itself succeeds because the token signature is still valid");
    let payloads = read_until_disconnected(&mut late).await;

    assert!(
        matches!(payloads.as_slice(), [Payload::RoomClosed(_)]),
        "should be told the room is closed and nothing else, got {payloads:?}"
    );
    assert_eq!(gateway.room_count(), 0, "닫힌 방이 다시 만들어지면 안 된다");
}

/// 같은 ID로 방을 다시 열면 회차가 올라간다. 새 회차로는 들어갈 수 있다.
#[tokio::test(flavor = "multi_thread")]
async fn reopened_room_accepts_the_new_generation_only() {
    const OLD: u64 = 1_795_000_000_000;
    const NEW: u64 = OLD + 5_000;
    let gateway = TestGateway::start(room_config(), session_config()).await;
    let room_id = unique_room_id("reopen-room");
    assert!(!gateway.close_room(&room_id, OLD));

    let mut newcomer = gateway.connect_in_generation(&room_id, "newcomer", NEW).await;
    send_text(&mut newcomer, "새 방입니다").await;
    let texts: Vec<String> = collect_until_quiet(&mut newcomer, QUIET_PERIOD)
        .await
        .into_iter()
        .filter_map(|payload| match payload {
            Payload::Batch(batch) => Some(batch.messages),
            _ => None,
        })
        .flatten()
        .map(|message| message.text)
        .collect();
    assert_eq!(texts, ["새 방입니다"]);

    let mut stale = gateway.connect_in_generation(&room_id, "stale", OLD).await;
    let payloads = read_until_disconnected(&mut stale).await;
    assert!(matches!(payloads.as_slice(), [Payload::RoomClosed(_)]));
}

/// 유효기간이 지난 방의 ID로 새 방이 열렸는데 정리 작업이 아직 돌지 않은 경우.
/// 이전 회차에 붙어 있던 사람이 새 방의 대화를 받으면 안 된다.
#[tokio::test(flavor = "multi_thread")]
async fn joining_a_newer_generation_evicts_members_of_the_old_one() {
    const OLD: u64 = 1_795_000_000_000;
    const NEW: u64 = OLD + 5_000;
    let gateway = TestGateway::start(room_config(), session_config()).await;
    let room_id = unique_room_id("evict-room");
    let mut old_member = gateway.connect_in_generation(&room_id, "old", OLD).await;
    wait_until("old member joined", WAIT_TIMEOUT, || {
        gateway.room_stats(&room_id).is_some_and(|stats| stats.connections == 1)
    })
    .await;

    let mut new_member = gateway.connect_in_generation(&room_id, "new", NEW).await;
    let old_payloads = read_until_disconnected(&mut old_member).await;
    assert!(
        matches!(old_payloads.last(), Some(Payload::RoomClosed(_))),
        "old member should be told the room closed, got {old_payloads:?}"
    );

    send_text(&mut new_member, "새 회차의 글").await;
    let delivered = collect_until_quiet(&mut new_member, QUIET_PERIOD).await;
    assert!(delivered.iter().any(|payload| matches!(payload, Payload::Batch(_))));
    assert!(
        !old_payloads.iter().any(|payload| matches!(payload, Payload::Batch(_))),
        "이전 회차 참여자는 새 방의 글을 받지 않는다"
    );
    assert_eq!(
        gateway.room_stats(&room_id).map(|stats| stats.connections),
        Some(1),
        "새 방에는 새 참여자만 남는다"
    );
}
