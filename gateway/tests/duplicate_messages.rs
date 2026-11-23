//! 짧은 시간에 같은 메시지가 몰릴 때의 동작.
//!
//! 지금 게이트웨이는 내용 기반 중복 차단을 하지 않는다. 방어선은 두 가지다.
//! - 연결별 rate limit: 한 사람이 같은 글을 연타하는 경우
//! - 방별 샘플링: 여러 사람이 같은 글을 동시에 올리는 경우
//!
//! 어느 경우든 통과한 메시지는 정확히 한 번만 전달되어야 한다.

mod support;

use std::time::Duration;

use chat_gateway::{
    config::{RoomConfig, SessionConfig},
    proto::chat::v1::{ChatMessage, ErrorCode, server_frame::Payload},
};
use futures_util::future::join_all;
use tokio_tungstenite::tungstenite;

use support::{TestGateway, collect_until_quiet, send_text, wait_until};

const FLUSH_INTERVAL: Duration = Duration::from_millis(50);
const QUIET_PERIOD: Duration = Duration::from_millis(400);
const WAIT_TIMEOUT: Duration = Duration::from_secs(5);
const RATE_LIMIT_BURST: u32 = 5;
const REPEATED_TEXT: &str = "ㅋㅋㅋㅋㅋ";

fn room_config(max_messages_per_flush: usize) -> RoomConfig {
    RoomConfig {
        flush_interval: FLUSH_INTERVAL,
        max_messages_per_flush,
        broadcast_capacity: 64,
    }
}

fn session_config() -> SessionConfig {
    SessionConfig {
        send_timeout: Duration::from_secs(5),
        rate_limit_burst: RATE_LIMIT_BURST,
        rate_limit_per_sec: 1.0,
        max_text_chars: 300,
        max_frame_bytes: 4096,
    }
}

fn split_payloads(payloads: Vec<Payload>) -> (Vec<ChatMessage>, Vec<i32>, Vec<usize>) {
    let mut messages = Vec::new();
    let mut error_codes = Vec::new();
    let mut batch_sizes = Vec::new();
    for payload in payloads {
        match payload {
            Payload::Batch(batch) => {
                batch_sizes.push(batch.messages.len());
                messages.extend(batch.messages);
            }
            Payload::Error(error) => error_codes.push(error.code),
            Payload::RoomClosed(_) => {}
        }
    }
    (messages, error_codes, batch_sizes)
}

fn seqs(messages: &[ChatMessage]) -> Vec<u64> {
    messages.iter().map(|message| message.seq).collect()
}

#[tokio::test(flavor = "multi_thread")]
async fn rejects_connection_with_invalid_token() {
    let gateway = TestGateway::start(room_config(4), session_config()).await;

    let result = gateway.connect_with_token("not-a-jwt").await;

    match result {
        Err(tungstenite::Error::Http(response)) => assert_eq!(response.status(), 401),
        other => panic!("expected HTTP 401, got {other:?}"),
    }
}

/// 한 사람이 같은 글을 20번 연타: burst만큼만 통과하고 나머지는 오류로 돌아온다.
#[tokio::test(flavor = "multi_thread")]
async fn same_user_repeating_text_is_cut_by_rate_limit() {
    const SENT: usize = 20;
    // 샘플링이 끼어들지 않게 주기당 상한을 넉넉히 둔다. 여기서는 rate limit만 본다.
    let gateway = TestGateway::start(room_config(100), session_config()).await;
    let mut spammer = gateway.connect("dup-room", "spammer").await;
    let mut viewer = gateway.connect("dup-room", "viewer").await;

    for _ in 0..SENT {
        send_text(&mut spammer, REPEATED_TEXT).await;
    }

    let (spammer_messages, spammer_errors, _) =
        split_payloads(collect_until_quiet(&mut spammer, QUIET_PERIOD).await);
    let (viewer_messages, viewer_errors, _) =
        split_payloads(collect_until_quiet(&mut viewer, QUIET_PERIOD).await);

    let burst = RATE_LIMIT_BURST as usize;
    assert_eq!(spammer_errors.len(), SENT - burst);
    assert!(
        spammer_errors
            .iter()
            .all(|code| *code == i32::from(ErrorCode::RateLimited))
    );
    assert!(viewer_errors.is_empty(), "오류는 보낸 사람에게만 간다");

    let expected_seqs: Vec<u64> = (1..=burst as u64).collect();
    assert_eq!(seqs(&viewer_messages), expected_seqs, "통과분은 한 번씩만 전달");
    assert_eq!(seqs(&spammer_messages), expected_seqs);
    assert!(
        viewer_messages
            .iter()
            .all(|message| message.text == REPEATED_TEXT && message.user_id == "spammer")
    );
}

/// 30명이 같은 글을 동시에 올림: 주기당 상한까지만 나가고 나머지는 샘플링으로 버려진다.
#[tokio::test(flavor = "multi_thread")]
async fn many_users_sending_same_text_are_sampled_without_duplicates() {
    const SENDERS: usize = 30;
    const VIEWERS: usize = 3;
    const MAX_PER_FLUSH: usize = 4;
    const ROOM: &str = "flood-room";
    let gateway = TestGateway::start(room_config(MAX_PER_FLUSH), session_config()).await;

    let mut viewers = Vec::new();
    for index in 0..VIEWERS {
        viewers.push(gateway.connect(ROOM, &format!("viewer-{index}")).await);
    }
    let mut senders = Vec::new();
    for index in 0..SENDERS {
        senders.push(gateway.connect(ROOM, &format!("sender-{index}")).await);
    }

    join_all(
        senders
            .iter_mut()
            .map(|sender| send_text(sender, REPEATED_TEXT)),
    )
    .await;

    // 30건이 전부 "전달 대상" 아니면 "샘플링으로 버림" 중 하나로 처리될 때까지 기다린다.
    wait_until("all sent messages accounted for", WAIT_TIMEOUT, || {
        gateway.room_stats(ROOM).is_some_and(|stats| {
            stats.last_seq + stats.sampled_out_messages == SENDERS as u64
        })
    })
    .await;
    let stats = gateway.room_stats(ROOM).expect("room should exist");
    let delivered = stats.last_seq;

    assert!(stats.sampled_out_messages > 0, "상한을 넘긴 만큼은 버려져야 한다");
    assert!(delivered >= MAX_PER_FLUSH as u64);

    let expected_seqs: Vec<u64> = (1..=delivered).collect();
    for viewer in &mut viewers {
        let (messages, errors, batch_sizes) =
            split_payloads(collect_until_quiet(viewer, QUIET_PERIOD).await);

        assert!(errors.is_empty());
        assert!(batch_sizes.iter().all(|size| *size <= MAX_PER_FLUSH));
        assert_eq!(seqs(&messages), expected_seqs, "모든 시청자가 같은 것을 한 번씩 본다");
        assert!(messages.iter().all(|message| message.text == REPEATED_TEXT));
    }
}

/// NUL 문자는 DB에 저장할 수 없다. 입구에서 거절한다.
#[tokio::test(flavor = "multi_thread")]
async fn text_containing_nul_is_rejected() {
    let gateway = TestGateway::start(room_config(100), session_config()).await;
    let mut sender = gateway.connect("nul-room", "sender").await;

    send_text(&mut sender, "앞\u{0}뒤").await;
    send_text(&mut sender, "정상 글").await;

    let (messages, errors, _) =
        split_payloads(collect_until_quiet(&mut sender, QUIET_PERIOD).await);
    assert_eq!(errors, [i32::from(ErrorCode::InvalidMessage)]);
    let texts: Vec<&str> = messages.iter().map(|message| message.text.as_str()).collect();
    assert_eq!(texts, ["정상 글"]);
}

/// rate limit은 연결 단위다. 같은 사용자가 연결을 새로 열면 버킷도 새로 받는다.
#[tokio::test(flavor = "multi_thread")]
async fn rate_limit_is_scoped_to_one_connection() {
    let gateway = TestGateway::start(room_config(100), session_config()).await;
    let mut first = gateway.connect("tab-room", "same-user").await;
    let mut second = gateway.connect("tab-room", "same-user").await;

    for _ in 0..RATE_LIMIT_BURST {
        send_text(&mut first, REPEATED_TEXT).await;
        send_text(&mut second, REPEATED_TEXT).await;
    }

    let (messages, errors, _) = split_payloads(collect_until_quiet(&mut first, QUIET_PERIOD).await);

    assert!(errors.is_empty());
    assert_eq!(messages.len(), 2 * RATE_LIMIT_BURST as usize);
}
