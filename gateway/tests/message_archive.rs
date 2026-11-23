//! 전체 기록 보관. 게이트웨이가 쓴 Parquet 파일을 실제로 다시 읽어서 확인한다.

mod support;

use std::{fs, path::Path, time::Duration};

use arrow_array::{Array, BooleanArray, Int64Array, RecordBatch, StringArray};
use chat_gateway::config::{RoomConfig, SessionConfig};
use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

use support::{TestGateway, send_text, wait_until};

const WAIT_TIMEOUT: Duration = Duration::from_secs(10);
const MAX_PER_FLUSH: usize = 4;
const ROOM: &str = "archive-room";

/// 보관 파일에서 읽은 한 줄.
#[derive(Debug)]
struct ArchivedRow {
    room_id: String,
    nickname: String,
    body: String,
    delivered: bool,
    seq: Option<i64>,
}

fn column<'a, T: 'static>(batch: &'a RecordBatch, name: &str) -> &'a T {
    batch
        .column_by_name(name)
        .unwrap_or_else(|| panic!("column {name} should exist"))
        .as_any()
        .downcast_ref::<T>()
        .unwrap_or_else(|| panic!("column {name} has an unexpected type"))
}

/// 폴더의 완성된 Parquet 파일을 전부 읽는다. 쓰는 중인 파일(.partial)은 건너뛴다.
fn read_archive(dir: &Path) -> (usize, Vec<ArchivedRow>) {
    let mut files: Vec<_> = fs::read_dir(dir)
        .expect("archive dir should exist")
        .map(|entry| entry.expect("dir entry should be readable").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "parquet"))
        .collect();
    files.sort();

    let mut rows = Vec::new();
    for path in &files {
        let file = fs::File::open(path).expect("archive file should open");
        let reader = ParquetRecordBatchReaderBuilder::try_new(file)
            .expect("archive file should be valid parquet")
            .build()
            .expect("archive reader should build");
        for batch in reader {
            let batch = batch.expect("record batch should decode");
            let room_ids: &StringArray = column(&batch, "room_id");
            let nicknames: &StringArray = column(&batch, "nickname");
            let bodies: &StringArray = column(&batch, "body");
            let delivered: &BooleanArray = column(&batch, "delivered");
            let seqs: &Int64Array = column(&batch, "seq");
            for index in 0..batch.num_rows() {
                rows.push(ArchivedRow {
                    room_id: room_ids.value(index).to_owned(),
                    nickname: nicknames.value(index).to_owned(),
                    body: bodies.value(index).to_owned(),
                    delivered: delivered.value(index),
                    seq: (!seqs.is_null(index)).then(|| seqs.value(index)),
                });
            }
        }
    }
    (files.len(), rows)
}

/// 화면에는 일부만 나가도 보관 파일에는 받은 글이 전부 남는다.
#[tokio::test(flavor = "multi_thread")]
async fn every_accepted_message_is_archived_including_sampled_out_ones() {
    const SENT: u64 = 60;
    let gateway = TestGateway::start(
        RoomConfig {
            flush_interval: Duration::from_millis(50),
            max_messages_per_flush: MAX_PER_FLUSH,
            broadcast_capacity: 64,
        },
        SessionConfig {
            send_timeout: Duration::from_secs(5),
            rate_limit_burst: 1000,
            rate_limit_per_sec: 1000.0,
            max_text_chars: 300,
            max_frame_bytes: 4096,
        },
    )
    .await;
    let mut sender = gateway.connect(ROOM, "flooder").await;

    for index in 0..SENT {
        send_text(&mut sender, &format!("flood {index}")).await;
    }
    wait_until("all sent messages accounted for", WAIT_TIMEOUT, || {
        gateway
            .room_stats(ROOM)
            .is_some_and(|stats| stats.last_seq + stats.sampled_out_messages == SENT)
    })
    .await;
    let stats = gateway.room_stats(ROOM).expect("room should exist");
    assert!(stats.sampled_out_messages > 0, "일부는 화면에 못 나가야 의미 있는 테스트다");

    wait_until("archive files written", WAIT_TIMEOUT, || {
        gateway.archive_stats().written_messages == SENT
    })
    .await;
    let (file_count, rows) = read_archive(gateway.archive_dir());

    assert!(file_count >= 1);
    assert_eq!(rows.len() as u64, SENT, "받은 글은 하나도 빠짐없이 보관된다");
    assert!(rows.iter().all(|row| row.room_id == ROOM && row.nickname == "flooder"));

    // 보낸 글이 전부, 보낸 순서대로 들어 있다.
    let bodies: Vec<&str> = rows.iter().map(|row| row.body.as_str()).collect();
    let expected: Vec<String> = (0..SENT).map(|index| format!("flood {index}")).collect();
    assert_eq!(bodies, expected);

    // 화면에 나간 것과 버려진 것이 게이트웨이 집계와 맞는다.
    let delivered: Vec<&ArchivedRow> = rows.iter().filter(|row| row.delivered).collect();
    assert_eq!(delivered.len() as u64, stats.last_seq);
    assert_eq!((rows.len() - delivered.len()) as u64, stats.sampled_out_messages);
    // 나간 것만 번호가 있고, 1부터 이어진다.
    let seqs: Vec<i64> = delivered.iter().filter_map(|row| row.seq).collect();
    assert_eq!(seqs, (1..=stats.last_seq as i64).collect::<Vec<_>>());
    assert!(rows.iter().filter(|row| !row.delivered).all(|row| row.seq.is_none()));

    assert_eq!(gateway.archive_stats().lost_messages, 0);
}
