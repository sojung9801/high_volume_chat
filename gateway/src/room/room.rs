use std::{
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use bytes::Bytes;
use prost::Message as _;
use serde::Serialize;
use tokio::{
    sync::{broadcast, watch},
    time::MissedTickBehavior,
};

use crate::{
    archive::{ArchivedMessage, MessageArchive},
    config::RoomConfig,
    proto::chat::v1::{ChatMessage, MessageBatch, ServerFrame, server_frame},
    store::MessageStore,
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RoomStats {
    pub room_id: String,
    pub connections: usize,
    pub last_seq: u64,
    /// 샘플링 상한을 넘어 버려진 메시지 수.
    pub sampled_out_messages: u64,
    /// 느린 연결이 따라오지 못해 건너뛴 프레임 수 (연결 합계).
    pub lagged_frames: u64,
}

struct PendingBatch {
    next_seq: u64,
    messages: Vec<ChatMessage>,
}

pub struct Room {
    id: String,
    /// 이 방의 회차. 회차 없는 토큰으로 처음 만들어진 방이면 `None`.
    generation: Option<u64>,
    config: RoomConfig,
    store: MessageStore,
    archive: MessageArchive,
    pending: Mutex<PendingBatch>,
    /// 인코딩된 프레임을 한 번만 만들고 `Bytes` 참조로 모든 연결에 공유한다.
    frames: broadcast::Sender<Bytes>,
    sampled_out_messages: AtomicU64,
    lagged_frames: AtomicU64,
    /// 방장이 방을 닫으면 true가 된다. 각 연결이 이 신호를 보고 스스로 끊는다.
    closed: watch::Sender<bool>,
}

/// `publish`에 넘기는 보낸 사람 정보.
pub struct Sender<'a> {
    pub user_id: &'a str,
    pub nickname: &'a str,
    pub is_owner: bool,
}

impl Room {
    /// 방을 만들고 flush 태스크를 띄운다. 태스크는 방이 drop되면 스스로 끝난다.
    pub fn spawn(
        id: String,
        generation: Option<u64>,
        config: RoomConfig,
        store: MessageStore,
        archive: MessageArchive,
    ) -> Arc<Self> {
        let (frames, _) = broadcast::channel(config.broadcast_capacity);
        let room = Arc::new(Self {
            id,
            generation,
            config,
            store,
            archive,
            pending: Mutex::new(PendingBatch {
                next_seq: 1,
                messages: Vec::with_capacity(config.max_messages_per_flush),
            }),
            frames,
            sampled_out_messages: AtomicU64::new(0),
            lagged_frames: AtomicU64::new(0),
            closed: watch::Sender::new(false),
        });

        tokio::spawn(run_flusher(Arc::downgrade(&room), config.flush_interval));
        room
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn generation(&self) -> Option<u64> {
        self.generation
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Bytes> {
        self.frames.subscribe()
    }

    pub fn subscriber_count(&self) -> usize {
        self.frames.receiver_count()
    }

    /// 다음 flush에 실릴 메시지를 쌓는다. 주기당 상한을 넘으면 버리고 `false`를 돌려준다.
    pub fn publish(&self, sender: Sender<'_>, text: String) -> bool {
        if self.is_closed() {
            return false;
        }

        let (sent_at_ms, delivered_seq) = {
            let mut pending = self.pending.lock().expect("room pending mutex poisoned");
            // 시각은 락 안에서 찍는다. 그래야 번호 순서와 시각 순서가 어긋나지 않는다.
            let sent_at_ms = now_ms();
            let delivered_seq = if pending.messages.len() >= self.config.max_messages_per_flush {
                None
            } else {
                let seq = pending.next_seq;
                pending.next_seq += 1;
                pending.messages.push(ChatMessage {
                    seq,
                    user_id: sender.user_id.to_owned(),
                    nickname: sender.nickname.to_owned(),
                    text: text.clone(),
                    sent_at_ms,
                    is_owner: sender.is_owner,
                });
                Some(seq)
            };
            (sent_at_ms, delivered_seq)
        };
        if delivered_seq.is_none() {
            self.sampled_out_messages.fetch_add(1, Ordering::Relaxed);
        }

        // 화면에 나가든 버려지든 받아들인 글은 전부 보관한다.
        self.archive.record(ArchivedMessage {
            room_id: self.id.clone(),
            sent_at_ms,
            user_id: sender.user_id.to_owned(),
            nickname: sender.nickname.to_owned(),
            text,
            is_owner: sender.is_owner,
            delivered_seq,
        });
        delivered_seq.is_some()
    }

    /// 방을 닫는다. 이후 메시지는 받지 않고, 붙어 있는 연결은 신호를 받고 끊는다.
    pub fn close(&self) {
        self.closed.send_replace(true);
    }

    pub fn is_closed(&self) -> bool {
        *self.closed.borrow()
    }

    pub fn closed_signal(&self) -> watch::Receiver<bool> {
        self.closed.subscribe()
    }

    pub fn record_lag(&self, skipped_frames: u64) {
        self.lagged_frames.fetch_add(skipped_frames, Ordering::Relaxed);
    }

    pub fn stats(&self) -> RoomStats {
        let last_seq = self
            .pending
            .lock()
            .expect("room pending mutex poisoned")
            .next_seq
            - 1;

        RoomStats {
            room_id: self.id.clone(),
            connections: self.subscriber_count(),
            last_seq,
            sampled_out_messages: self.sampled_out_messages.load(Ordering::Relaxed),
            lagged_frames: self.lagged_frames.load(Ordering::Relaxed),
        }
    }

    fn flush(&self) {
        let messages = {
            let mut pending = self.pending.lock().expect("room pending mutex poisoned");
            if pending.messages.is_empty() {
                return;
            }
            std::mem::replace(
                &mut pending.messages,
                Vec::with_capacity(self.config.max_messages_per_flush),
            )
        };

        // 시청자에게 나가는 것만 저장한다. 큐에 넣기만 하고 DB는 기다리지 않는다.
        self.store.enqueue(&self.id, &messages);

        let frame = ServerFrame {
            payload: Some(server_frame::Payload::Batch(MessageBatch { messages })),
        };

        // 구독자가 없으면 Err. 그 경우 버려도 된다.
        let _ = self.frames.send(Bytes::from(frame.encode_to_vec()));
    }
}

/// 마지막 사람이 나가면 방이 사라진다. 그 직전 200ms 안에 받은 글은 아직 내보내기 전이라
/// 대기열에 남아 있다. 받을 사람은 없지만 받아들인 글이므로 기록에는 남긴다.
impl Drop for Room {
    fn drop(&mut self) {
        let pending = self
            .pending
            .get_mut()
            .expect("room pending mutex poisoned");
        if !pending.messages.is_empty() {
            self.store.enqueue(&self.id, &pending.messages);
        }
    }
}

async fn run_flusher(room: Weak<Room>, period: Duration) {
    let mut ticker = tokio::time::interval(period);
    ticker.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        ticker.tick().await;
        let Some(room) = room.upgrade() else {
            break;
        };
        room.flush();
    }
}

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| i64::try_from(elapsed.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}
