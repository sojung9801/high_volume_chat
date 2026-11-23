//! 메시지 저장. 실제 Postgres(`docker compose up -d`)에 쓴 뒤 직접 읽어서 확인한다.

mod support;

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chat_gateway::{
    config::{RoomConfig, SessionConfig},
    proto::chat::v1::server_frame::Payload,
};
use sqlx::{PgPool, Row};

use support::{
    TestGateway, collect_until_quiet, send_text, store_config, test_database_url, wait_until,
};

const QUIET_PERIOD: Duration = Duration::from_millis(400);
const WAIT_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_PER_FLUSH: usize = 4;

fn room_config() -> RoomConfig {
    RoomConfig {
        flush_interval: Duration::from_millis(50),
        max_messages_per_flush: MAX_PER_FLUSH,
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

/// 실행마다 다른 방 ID. 이전 실행이 남긴 행과 섞이지 않는다.
fn unique_room_id(prefix: &str) -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after epoch")
        .as_nanos();
    format!("{prefix}-{nanos}")
}

async fn stored_rows(pool: &PgPool, room_id: &str) -> Vec<(i64, String, String, String)> {
    sqlx::query(
        "SELECT seq, user_id, nickname, body FROM chat_messages \
         WHERE room_id = $1 ORDER BY sent_at_ms, seq",
    )
    .bind(room_id)
    .fetch_all(pool)
    .await
    .expect("stored rows should load")
    .into_iter()
    .map(|row| (row.get(0), row.get(1), row.get(2), row.get(3)))
    .collect()
}

async fn wait_for_row_count(pool: &PgPool, room_id: &str, expected: usize) {
    let deadline = tokio::time::Instant::now() + WAIT_TIMEOUT;
    loop {
        let count = stored_rows(pool, room_id).await.len();
        if count == expected {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "expected {expected} stored rows, found {count}"
        );
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
}

fn delivered_texts(payloads: Vec<Payload>) -> Vec<String> {
    payloads
        .into_iter()
        .filter_map(|payload| match payload {
            Payload::Batch(batch) => Some(batch.messages),
            Payload::Error(_) | Payload::RoomClosed(_) => None,
        })
        .flatten()
        .map(|message| message.text)
        .collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn delivered_messages_are_stored_in_order() {
    let pool = PgPool::connect(&test_database_url())
        .await
        .expect("test database should be reachable (docker compose up -d)");
    let gateway = TestGateway::start(room_config(), session_config()).await;
    let room_id = unique_room_id("store-room");
    let mut sender = gateway.connect(&room_id, "writer").await;

    let texts = ["첫 번째", "두 번째", "세 번째"];
    for text in texts {
        send_text(&mut sender, text).await;
        // flush 주기보다 길게 띄워서 샘플링에 걸리지 않게 한다.
        tokio::time::sleep(Duration::from_millis(80)).await;
    }
    let delivered = delivered_texts(collect_until_quiet(&mut sender, QUIET_PERIOD).await);
    assert_eq!(delivered, texts);

    wait_for_row_count(&pool, &room_id, texts.len()).await;
    let rows = stored_rows(&pool, &room_id).await;

    for (index, (seq, user_id, nickname, body)) in rows.iter().enumerate() {
        assert_eq!(*seq, index as i64 + 1);
        assert_eq!(user_id, "writer");
        assert_eq!(nickname, "writer");
        assert_eq!(body, texts[index]);
    }
    assert_eq!(gateway.store_stats().unsaved_messages, 0);
}

/// 샘플링으로 버려진 메시지는 저장하지 않는다. 시청자가 본 것과 DB가 같아야 한다.
#[tokio::test(flavor = "multi_thread")]
async fn sampled_out_messages_are_not_stored() {
    const SENT: u64 = 40;
    let pool = PgPool::connect(&test_database_url())
        .await
        .expect("test database should be reachable (docker compose up -d)");
    let gateway = TestGateway::start(room_config(), session_config()).await;
    let room_id = unique_room_id("sampled-room");
    let mut sender = gateway.connect(&room_id, "flooder").await;

    for index in 0..SENT {
        send_text(&mut sender, &format!("flood {index}")).await;
    }
    wait_until("all sent messages accounted for", WAIT_TIMEOUT, || {
        gateway
            .room_stats(&room_id)
            .is_some_and(|stats| stats.last_seq + stats.sampled_out_messages == SENT)
    })
    .await;
    let stats = gateway.room_stats(&room_id).expect("room should exist");
    let delivered = delivered_texts(collect_until_quiet(&mut sender, QUIET_PERIOD).await);

    assert!(stats.sampled_out_messages > 0);
    wait_for_row_count(&pool, &room_id, delivered.len()).await;
    let stored: Vec<String> = stored_rows(&pool, &room_id)
        .await
        .into_iter()
        .map(|(_, _, _, body)| body)
        .collect();
    assert_eq!(stored, delivered);
}

/// DB에 닿지 못해도 채팅은 그대로 전달된다. 저장 실패만 집계된다.
#[tokio::test(flavor = "multi_thread")]
async fn chat_keeps_flowing_when_database_is_unreachable() {
    // 아무도 듣지 않는 포트. 연결이 즉시 거부된다.
    let unreachable = "postgres://chat:chat@127.0.0.1:1/chat".to_owned();
    let gateway =
        TestGateway::start_with_store(room_config(), session_config(), store_config(unreachable))
            .await;
    let room_id = unique_room_id("no-db-room");
    let mut sender = gateway.connect(&room_id, "writer").await;
    let mut viewer = gateway.connect(&room_id, "viewer").await;

    send_text(&mut sender, "DB 없이도 전달").await;

    let delivered = delivered_texts(collect_until_quiet(&mut viewer, QUIET_PERIOD).await);
    assert_eq!(delivered, ["DB 없이도 전달"]);
    wait_until("store failure counted", WAIT_TIMEOUT, || {
        gateway.store_stats().unsaved_messages == 1
    })
    .await;
}

/// 마지막 사람이 글을 보내자마자 나가도 그 글은 저장된다 (아직 내보내기 전이었더라도).
#[tokio::test(flavor = "multi_thread")]
async fn message_sent_just_before_the_room_empties_is_still_stored() {
    let pool = PgPool::connect(&test_database_url())
        .await
        .expect("test database should be reachable (docker compose up -d)");
    // flush 주기를 길게 둬서 글이 대기열에 남아 있는 동안 방이 비게 만든다.
    let slow_flush = RoomConfig {
        flush_interval: Duration::from_secs(30),
        ..room_config()
    };
    let gateway = TestGateway::start(slow_flush, session_config()).await;
    let room_id = unique_room_id("last-words-room");
    let mut sender = gateway.connect(&room_id, "leaver").await;

    send_text(&mut sender, "나가기 직전의 글").await;
    wait_until("message accepted", WAIT_TIMEOUT, || {
        gateway.room_stats(&room_id).is_some_and(|stats| stats.last_seq == 1)
    })
    .await;
    drop(sender);
    wait_until("room removed", WAIT_TIMEOUT, || gateway.room_count() == 0).await;

    wait_for_row_count(&pool, &room_id, 1).await;
    let rows = stored_rows(&pool, &room_id).await;
    assert_eq!(rows[0].3, "나가기 직전의 글");
}
