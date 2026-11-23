use std::{
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use anyhow::Result;
use serde::Serialize;
use sqlx::{PgPool, postgres::PgPoolOptions};
use tokio::sync::mpsc;

use crate::{config::StoreConfig, proto::chat::v1::ChatMessage};

/// DB가 응답하지 않을 때 한 번의 저장 시도가 붙잡혀 있는 최대 시간.
const ACQUIRE_TIMEOUT: Duration = Duration::from_secs(3);

const INSERT_MESSAGES_SQL: &str = "\
    INSERT INTO chat_messages (room_id, sent_at_ms, seq, user_id, nickname, body, is_owner) \
    SELECT * FROM UNNEST($1::text[], $2::bigint[], $3::bigint[], $4::text[], $5::text[], $6::text[], $7::boolean[]) \
    ON CONFLICT DO NOTHING";

struct QueuedMessage {
    room_id: String,
    message: ChatMessage,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoreStats {
    /// 큐가 가득 찼거나 DB 쓰기가 실패해서 저장하지 못한 메시지 수.
    pub unsaved_messages: u64,
}

/// 내보낸 메시지를 DB에 기록한다.
///
/// 전송 경로와 분리되어 있다. 방은 큐에 넣기만 하고 기다리지 않는다.
/// DB가 느리거나 죽으면 저장만 포기하고 채팅은 계속 간다.
#[derive(Clone)]
pub struct MessageStore {
    queue: mpsc::Sender<QueuedMessage>,
    unsaved_messages: Arc<AtomicU64>,
}

impl MessageStore {
    /// 풀을 만들고 쓰기 태스크를 띄운다. 실제 연결은 첫 쓰기 때 맺는다.
    /// 그래서 DB가 내려가 있어도 게이트웨이는 뜬다.
    pub fn start(config: &StoreConfig) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(config.max_connections)
            .acquire_timeout(ACQUIRE_TIMEOUT)
            .connect_lazy(&config.database_url)?;
        let (queue, receiver) = mpsc::channel(config.queue_capacity);
        let unsaved_messages = Arc::new(AtomicU64::new(0));

        tokio::spawn(run_writer(
            pool,
            receiver,
            config.batch_max_messages,
            Arc::clone(&unsaved_messages),
        ));

        Ok(Self {
            queue,
            unsaved_messages,
        })
    }

    /// 저장 큐에 넣는다. 큐가 가득 차면 버리고 센다. 절대 기다리지 않는다.
    pub fn enqueue(&self, room_id: &str, messages: &[ChatMessage]) {
        for message in messages {
            let queued = QueuedMessage {
                room_id: room_id.to_owned(),
                message: message.clone(),
            };
            if self.queue.try_send(queued).is_err() {
                self.unsaved_messages.fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    pub fn stats(&self) -> StoreStats {
        StoreStats {
            unsaved_messages: self.unsaved_messages.load(Ordering::Relaxed),
        }
    }
}

async fn run_writer(
    pool: PgPool,
    mut receiver: mpsc::Receiver<QueuedMessage>,
    batch_max_messages: usize,
    unsaved_messages: Arc<AtomicU64>,
) {
    let mut batch = Vec::with_capacity(batch_max_messages);

    // 쌓인 만큼 한 번에 꺼내 INSERT 한 번으로 쓴다. 모든 송신자가 사라지면 끝난다.
    while receiver.recv_many(&mut batch, batch_max_messages).await > 0 {
        if let Err(err) = insert_batch(&pool, &batch).await {
            unsaved_messages.fetch_add(batch.len() as u64, Ordering::Relaxed);
            tracing::warn!(%err, count = batch.len(), "failed to store chat messages");
        }
        batch.clear();
    }
}

async fn insert_batch(pool: &PgPool, batch: &[QueuedMessage]) -> Result<(), sqlx::Error> {
    let mut room_ids = Vec::with_capacity(batch.len());
    let mut sent_at_ms = Vec::with_capacity(batch.len());
    let mut seqs = Vec::with_capacity(batch.len());
    let mut user_ids = Vec::with_capacity(batch.len());
    let mut nicknames = Vec::with_capacity(batch.len());
    let mut bodies = Vec::with_capacity(batch.len());
    let mut owner_flags = Vec::with_capacity(batch.len());
    for queued in batch {
        room_ids.push(queued.room_id.as_str());
        sent_at_ms.push(queued.message.sent_at_ms);
        seqs.push(i64::try_from(queued.message.seq).unwrap_or(i64::MAX));
        user_ids.push(queued.message.user_id.as_str());
        nicknames.push(queued.message.nickname.as_str());
        bodies.push(queued.message.text.as_str());
        owner_flags.push(queued.message.is_owner);
    }

    sqlx::query(INSERT_MESSAGES_SQL)
        .bind(room_ids)
        .bind(sent_at_ms)
        .bind(seqs)
        .bind(user_ids)
        .bind(nicknames)
        .bind(bodies)
        .bind(owner_flags)
        .execute(pool)
        .await
        .map(|_| ())
}
