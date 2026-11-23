use std::{
    fs::{self, File},
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

use anyhow::{Context, Result};
use arrow_array::{ArrayRef, BooleanArray, Int64Array, RecordBatch, StringArray};
use arrow_schema::{DataType, Field, Schema, SchemaRef};
use parquet::{
    arrow::ArrowWriter,
    basic::{Compression, ZstdLevel},
    file::properties::WriterProperties,
};
use serde::Serialize;
use tokio::{sync::mpsc, time::Instant};

use crate::config::ArchiveConfig;

const FILE_EXTENSION: &str = "parquet";
/// 쓰는 중인 파일에 붙이는 확장자. 다 쓴 뒤에 이름을 바꾼다.
const PARTIAL_EXTENSION: &str = "partial";

/// 보관할 메시지 한 건. 화면에 나가지 못한 것도 포함한다.
pub struct ArchivedMessage {
    pub room_id: String,
    pub sent_at_ms: i64,
    pub user_id: String,
    pub nickname: String,
    pub text: String,
    pub is_owner: bool,
    /// 시청자에게 나갔으면 그때 받은 방 안 번호. 샘플링으로 버려졌으면 `None`.
    pub delivered_seq: Option<u64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveStats {
    pub written_files: u64,
    pub written_messages: u64,
    /// 대기줄이 가득 찼거나 파일 쓰기가 실패해서 보관하지 못한 메시지 수.
    pub lost_messages: u64,
}

#[derive(Default)]
struct Counters {
    written_files: AtomicU64,
    written_messages: AtomicU64,
    lost_messages: AtomicU64,
}

/// 받아들인 메시지를 전부 Parquet 파일로 쌓는다.
///
/// DB 저장(`MessageStore`)은 화면에 나간 것만 다루고 조회가 빨라야 한다.
/// 이쪽은 버려진 것까지 전부 남기는 대신 조회는 나중에 파일을 읽어서 한다.
///
/// Parquet는 파일 끝에 덧붙일 수 없다. 그래서 일정 시간(또는 일정 건수)을 모아
/// 파일 하나로 쓰고 닫는다. 전송 경로는 대기줄에 넣기만 하고 기다리지 않는다.
#[derive(Clone)]
pub struct MessageArchive {
    queue: mpsc::Sender<ArchivedMessage>,
    counters: Arc<Counters>,
}

impl MessageArchive {
    pub fn start(config: &ArchiveConfig) -> Result<Self> {
        fs::create_dir_all(&config.dir)
            .with_context(|| format!("cannot create archive dir {}", config.dir.display()))?;

        let (queue, receiver) = mpsc::channel(config.queue_capacity);
        let counters = Arc::new(Counters::default());
        tokio::spawn(run_writer(config.clone(), receiver, Arc::clone(&counters)));

        Ok(Self { queue, counters })
    }

    /// 보관 대기줄에 넣는다. 가득 차면 버리고 센다. 절대 기다리지 않는다.
    pub fn record(&self, message: ArchivedMessage) {
        if self.queue.try_send(message).is_err() {
            self.counters.lost_messages.fetch_add(1, Ordering::Relaxed);
        }
    }

    pub fn stats(&self) -> ArchiveStats {
        ArchiveStats {
            written_files: self.counters.written_files.load(Ordering::Relaxed),
            written_messages: self.counters.written_messages.load(Ordering::Relaxed),
            lost_messages: self.counters.lost_messages.load(Ordering::Relaxed),
        }
    }
}

async fn run_writer(
    config: ArchiveConfig,
    mut receiver: mpsc::Receiver<ArchivedMessage>,
    counters: Arc<Counters>,
) {
    let mut pending: Vec<ArchivedMessage> = Vec::new();
    let mut file_number = 0u64;
    let mut deadline = Instant::now() + config.flush_interval;

    loop {
        // 가득 차면 바로 쓰고 비우므로 여기서는 항상 자리가 남아 있다.
        let room_left = config.max_messages_per_file - pending.len();
        let is_open = tokio::select! {
            received = receiver.recv_many(&mut pending, room_left) => received > 0,
            () = tokio::time::sleep_until(deadline) => true,
        };

        // 파일을 닫는 조건: 시간이 됐거나, 파일 하나에 담을 만큼 찼거나, 더 올 것이 없거나.
        let is_due = Instant::now() >= deadline;
        let is_full = pending.len() >= config.max_messages_per_file;
        if is_due || is_full || !is_open {
            if !pending.is_empty() {
                file_number += 1;
                let messages = std::mem::take(&mut pending);
                write_file(&config.dir, file_number, messages, &counters).await;
            }
            deadline = Instant::now() + config.flush_interval;
        }
        if !is_open {
            break;
        }
    }
}

async fn write_file(
    dir: &Path,
    file_number: u64,
    messages: Vec<ArchivedMessage>,
    counters: &Counters,
) {
    let count = messages.len() as u64;
    let first_sent_at_ms = messages.first().map_or(0, |message| message.sent_at_ms);
    let path = dir.join(format!("chat-{first_sent_at_ms}-{file_number:06}.{FILE_EXTENSION}"));

    // 파일 쓰기는 오래 걸릴 수 있는 작업이라 채팅을 처리하는 스레드에서 하지 않는다.
    let target = path.clone();
    let result = tokio::task::spawn_blocking(move || write_parquet(&target, &messages)).await;

    match result {
        Ok(Ok(())) => {
            counters.written_files.fetch_add(1, Ordering::Relaxed);
            counters.written_messages.fetch_add(count, Ordering::Relaxed);
        }
        Ok(Err(err)) => {
            counters.lost_messages.fetch_add(count, Ordering::Relaxed);
            tracing::warn!(%err, count, path = %path.display(), "failed to write archive file");
        }
        Err(err) => {
            counters.lost_messages.fetch_add(count, Ordering::Relaxed);
            tracing::warn!(%err, count, "archive write task failed");
        }
    }
}

fn archive_schema() -> SchemaRef {
    Arc::new(Schema::new(vec![
        Field::new("room_id", DataType::Utf8, false),
        Field::new("sent_at_ms", DataType::Int64, false),
        Field::new("user_id", DataType::Utf8, false),
        Field::new("nickname", DataType::Utf8, false),
        Field::new("body", DataType::Utf8, false),
        Field::new("is_owner", DataType::Boolean, false),
        Field::new("delivered", DataType::Boolean, false),
        // 화면에 나간 메시지만 번호가 있다.
        Field::new("seq", DataType::Int64, true),
    ]))
}

/// 메시지 묶음을 Parquet 파일 하나로 쓴다.
///
/// Parquet는 줄 단위가 아니라 칸 단위로 저장한다. 그래서 메시지 목록을
/// "칸별 배열"로 다시 묶어서 넘긴다.
fn write_parquet(path: &Path, messages: &[ArchivedMessage]) -> Result<()> {
    let schema = archive_schema();
    let columns: Vec<ArrayRef> = vec![
        Arc::new(StringArray::from_iter_values(messages.iter().map(|m| m.room_id.as_str()))),
        Arc::new(Int64Array::from_iter_values(messages.iter().map(|m| m.sent_at_ms))),
        Arc::new(StringArray::from_iter_values(messages.iter().map(|m| m.user_id.as_str()))),
        Arc::new(StringArray::from_iter_values(messages.iter().map(|m| m.nickname.as_str()))),
        Arc::new(StringArray::from_iter_values(messages.iter().map(|m| m.text.as_str()))),
        Arc::new(BooleanArray::from_iter(messages.iter().map(|m| Some(m.is_owner)))),
        Arc::new(BooleanArray::from_iter(
            messages.iter().map(|m| Some(m.delivered_seq.is_some())),
        )),
        Arc::new(Int64Array::from_iter(messages.iter().map(|m| {
            m.delivered_seq.map(|seq| i64::try_from(seq).unwrap_or(i64::MAX))
        }))),
    ];
    let batch = RecordBatch::try_new(Arc::clone(&schema), columns)?;

    let properties = WriterProperties::builder()
        .set_compression(Compression::ZSTD(ZstdLevel::default()))
        .build();

    // 다 쓰기 전의 파일이 읽히지 않게 임시 이름으로 쓰고, 끝나면 이름을 바꾼다.
    let partial_path: PathBuf = path.with_extension(PARTIAL_EXTENSION);
    let mut writer = ArrowWriter::try_new(File::create(&partial_path)?, schema, Some(properties))?;
    writer.write(&batch)?;
    writer.close()?;
    fs::rename(&partial_path, path)?;
    Ok(())
}
