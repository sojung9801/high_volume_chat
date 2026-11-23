//! 대용량 시나리오. 시청자 다수 + 송신자가 상한보다 훨씬 많이 쏟아붓는 상황.
//!
//! 규모는 환경 변수로 키운다.
//!   HIGH_VOLUME_VIEWERS=5000 HIGH_VOLUME_SENDERS=200 HIGH_VOLUME_SECONDS=10 \
//!     cargo test --release --test high_volume -- --nocapture

mod support;

use std::{
    env,
    str::FromStr,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use chat_gateway::{
    config::{RoomConfig, SessionConfig},
    proto::chat::v1::server_frame::Payload,
};
use futures_util::{SinkExt, StreamExt, future::join_all};
use tokio::{sync::watch, task::JoinHandle};

use support::{ClientSocket, TestGateway, decode_server_frame, encode_send_message, wait_until};

const ROOM: &str = "live-room";
const FLUSH_INTERVAL: Duration = Duration::from_millis(50);
const MAX_PER_FLUSH: usize = 4;
const SEND_INTERVAL: Duration = Duration::from_millis(10);
const CONNECT_CHUNK: usize = 100;
const WAIT_TIMEOUT: Duration = Duration::from_secs(30);
/// 타이머 지터를 감안한 프레임 수 여유.
const FRAME_COUNT_SLACK: f64 = 1.25;

struct Scale {
    viewers: usize,
    senders: usize,
    duration: Duration,
}

struct ViewerReport {
    seqs: Vec<u64>,
    frames: usize,
    max_batch: usize,
    latencies_ms: Vec<i64>,
}

fn env_or<T: FromStr>(key: &str, default: T) -> T {
    match env::var(key) {
        Ok(raw) => raw
            .parse()
            .unwrap_or_else(|_| panic!("{key} has an invalid value {raw:?}")),
        Err(_) => default,
    }
}

fn now_ms() -> i64 {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock should be after epoch");
    i64::try_from(elapsed.as_millis()).expect("timestamp should fit in i64")
}

fn percentile(sorted: &[i64], ratio: f64) -> i64 {
    let index = ((sorted.len() - 1) as f64 * ratio).round() as usize;
    sorted[index]
}

async fn connect_many(gateway: &TestGateway, prefix: &str, count: usize) -> Vec<ClientSocket> {
    let ids: Vec<String> = (0..count).map(|index| format!("{prefix}-{index}")).collect();
    let mut sockets = Vec::with_capacity(count);
    // 한꺼번에 열면 listen backlog가 넘친다. 나눠서 연다.
    for chunk in ids.chunks(CONNECT_CHUNK) {
        sockets.extend(join_all(chunk.iter().map(|id| gateway.connect(ROOM, id))).await);
    }
    sockets
}

fn spawn_viewer(
    mut socket: ClientSocket,
    mut stop: watch::Receiver<bool>,
    received: Arc<AtomicU64>,
) -> JoinHandle<ViewerReport> {
    tokio::spawn(async move {
        let mut report = ViewerReport {
            seqs: Vec::new(),
            frames: 0,
            max_batch: 0,
            latencies_ms: Vec::new(),
        };

        loop {
            tokio::select! {
                _ = stop.changed() => break,
                message = socket.next() => {
                    let Some(Ok(message)) = message else { break };
                    let Some(Payload::Batch(batch)) = decode_server_frame(&message) else { continue };
                    let arrived_at = now_ms();
                    report.frames += 1;
                    report.max_batch = report.max_batch.max(batch.messages.len());
                    received.fetch_add(batch.messages.len() as u64, Ordering::Relaxed);
                    for message in batch.messages {
                        report.seqs.push(message.seq);
                        report.latencies_ms.push(arrived_at - message.sent_at_ms);
                    }
                }
            }
        }
        report
    })
}

fn spawn_sender(
    socket: ClientSocket,
    duration: Duration,
    mut stop: watch::Receiver<bool>,
    sent: Arc<AtomicU64>,
    finished_senders: Arc<AtomicU64>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        let (mut sink, mut stream) = socket.split();
        // 송신자도 방 구성원이라 프레임을 받는다. 읽어서 버려야 서버 쪽 전송이 안 막힌다.
        let drain = tokio::spawn(async move { while let Some(Ok(_)) = stream.next().await {} });

        let message = encode_send_message("대용량 테스트 메시지");
        let mut ticker = tokio::time::interval(SEND_INTERVAL);
        let started = Instant::now();
        while started.elapsed() < duration {
            ticker.tick().await;
            sink.send(message.clone())
                .await
                .expect("sender frame should send");
            sent.fetch_add(1, Ordering::Relaxed);
        }
        finished_senders.fetch_add(1, Ordering::Relaxed);

        // 끝까지 방에 남아 있는다. 먼저 나가면 연결 수가 바뀐다.
        let _ = stop.changed().await;
        drain.abort();
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn fan_out_stays_bounded_and_consistent_under_flood() {
    let scale = Scale {
        viewers: env_or("HIGH_VOLUME_VIEWERS", 300),
        senders: env_or("HIGH_VOLUME_SENDERS", 30),
        duration: Duration::from_secs(env_or("HIGH_VOLUME_SECONDS", 3)),
    };
    let gateway = TestGateway::start(
        RoomConfig {
            flush_interval: FLUSH_INTERVAL,
            max_messages_per_flush: MAX_PER_FLUSH,
            broadcast_capacity: 64,
        },
        // rate limit을 사실상 풀어서 방 샘플링이 유일한 방어선이 되게 한다.
        SessionConfig {
            send_timeout: Duration::from_secs(5),
            rate_limit_burst: 1_000_000,
            rate_limit_per_sec: 1_000_000.0,
            max_text_chars: 300,
            max_frame_bytes: 4096,
        },
    )
    .await;

    let viewer_sockets = connect_many(&gateway, "viewer", scale.viewers).await;
    let sender_sockets = connect_many(&gateway, "sender", scale.senders).await;
    let connections = scale.viewers + scale.senders;
    assert_eq!(
        gateway.room_stats(ROOM).map(|stats| stats.connections),
        Some(connections)
    );

    let (stop_tx, stop_rx) = watch::channel(false);
    let received = Arc::new(AtomicU64::new(0));
    let sent = Arc::new(AtomicU64::new(0));
    let finished_senders = Arc::new(AtomicU64::new(0));
    let viewers: Vec<_> = viewer_sockets
        .into_iter()
        .map(|socket| spawn_viewer(socket, stop_rx.clone(), Arc::clone(&received)))
        .collect();

    let started = Instant::now();
    let senders: Vec<_> = sender_sockets
        .into_iter()
        .map(|socket| {
            spawn_sender(
                socket,
                scale.duration,
                stop_rx.clone(),
                Arc::clone(&sent),
                Arc::clone(&finished_senders),
            )
        })
        .collect();

    wait_until("senders finished sending", scale.duration + WAIT_TIMEOUT, || {
        finished_senders.load(Ordering::Relaxed) == scale.senders as u64
    })
    .await;
    // 보낸 것이 전부 "전달 대상" 아니면 "샘플링으로 버림"으로 처리될 때까지.
    let total_sent = sent.load(Ordering::Relaxed);
    wait_until("gateway processed every sent message", WAIT_TIMEOUT, || {
        gateway
            .room_stats(ROOM)
            .is_some_and(|stats| stats.last_seq + stats.sampled_out_messages == total_sent)
    })
    .await;
    let stats = gateway.room_stats(ROOM).expect("room should exist");
    let delivered = stats.last_seq;

    // 마지막 flush까지 모든 시청자에게 도착할 때까지.
    let expected_received = delivered * scale.viewers as u64;
    wait_until("every viewer received every delivered message", WAIT_TIMEOUT, || {
        received.load(Ordering::Relaxed) == expected_received
    })
    .await;
    let elapsed = started.elapsed();

    stop_tx.send(true).expect("tasks should still be listening");
    let reports: Vec<ViewerReport> = join_all(viewers)
        .await
        .into_iter()
        .map(|report| report.expect("viewer task should not panic"))
        .collect();
    join_all(senders).await;

    let expected_seqs: Vec<u64> = (1..=delivered).collect();
    let max_frames =
        (elapsed.as_secs_f64() / FLUSH_INTERVAL.as_secs_f64() * FRAME_COUNT_SLACK).ceil() as usize;

    assert!(stats.sampled_out_messages > 0, "유입이 상한보다 많으니 버려진 것이 있어야 한다");
    assert_eq!(stats.lagged_frames, 0, "이 규모에서 뒤처지는 연결이 있으면 안 된다");
    for report in &reports {
        assert_eq!(report.seqs, expected_seqs, "중복, 누락, 순서 뒤바뀜이 없어야 한다");
        assert!(report.max_batch <= MAX_PER_FLUSH);
        assert!(
            report.frames <= max_frames,
            "frames {} exceeded bound {max_frames}",
            report.frames
        );
    }

    let mut latencies: Vec<i64> = reports
        .iter()
        .flat_map(|report| report.latencies_ms.iter().copied())
        .collect();
    latencies.sort_unstable();
    let seconds = elapsed.as_secs_f64();
    println!("--- high volume report ---");
    println!("connections          {connections} (viewers {}, senders {})", scale.viewers, scale.senders);
    println!("elapsed              {seconds:.2}s");
    println!("inbound              {total_sent} msgs ({:.0}/s)", total_sent as f64 / seconds);
    println!("delivered per viewer {delivered} msgs ({:.1}/s)", delivered as f64 / seconds);
    println!(
        "sampled out          {} ({:.1}%)",
        stats.sampled_out_messages,
        stats.sampled_out_messages as f64 / total_sent as f64 * 100.0
    );
    println!("fan-out deliveries   {expected_received} ({:.0}/s)", expected_received as f64 / seconds);
    println!(
        "latency ms           p50 {} / p99 {} / max {}",
        percentile(&latencies, 0.50),
        percentile(&latencies, 0.99),
        percentile(&latencies, 1.0)
    );

    // 모두 나가면 방이 정리된다.
    drop(reports);
    wait_until("empty room removed", WAIT_TIMEOUT, || gateway.room_count() == 0).await;
}
