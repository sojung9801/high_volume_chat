use std::{env, fmt::Display, net::SocketAddr, path::PathBuf, str::FromStr, time::Duration};

use anyhow::{Context, Result, anyhow, ensure};

const MIN_JOIN_TOKEN_SECRET_LEN: usize = 32;

pub struct Config {
    /// 클라이언트 WebSocket을 받는 주소.
    pub public_addr: SocketAddr,
    /// 관리 API만 접근하는 내부 주소. 외부에 노출하지 않는다.
    pub internal_addr: SocketAddr,
    pub join_token_secret: String,
    pub room: RoomConfig,
    pub session: SessionConfig,
    pub store: StoreConfig,
    pub archive: ArchiveConfig,
}

#[derive(Clone)]
pub struct ArchiveConfig {
    /// Parquet 파일을 쌓는 폴더.
    pub dir: PathBuf,
    /// 이 시간마다 모인 것을 파일 하나로 쓴다.
    pub flush_interval: Duration,
    /// 파일 하나에 담는 최대 메시지 수. 시간이 안 됐어도 이만큼 차면 쓴다.
    pub max_messages_per_file: usize,
    /// 보관 대기 메시지 수. 넘치면 보관을 포기한다 (전송에는 영향 없음).
    pub queue_capacity: usize,
}

#[derive(Clone)]
pub struct StoreConfig {
    pub database_url: String,
    /// 저장 대기 메시지 수. 넘치면 저장을 포기한다 (전송에는 영향 없음).
    pub queue_capacity: usize,
    /// INSERT 한 번에 묶는 최대 메시지 수.
    pub batch_max_messages: usize,
    pub max_connections: u32,
}

#[derive(Clone, Copy)]
pub struct RoomConfig {
    /// 방 메시지를 묶어서 내보내는 주기.
    pub flush_interval: Duration,
    /// 한 주기에 내보내는 최대 메시지 수. 초과분은 샘플링으로 버린다.
    pub max_messages_per_flush: usize,
    /// 연결별 대기 프레임 수. 넘치면 느린 연결이 오래된 프레임을 잃는다.
    pub broadcast_capacity: usize,
}

#[derive(Clone, Copy)]
pub struct SessionConfig {
    /// 이 시간 안에 프레임을 못 받는 연결은 끊는다.
    pub send_timeout: Duration,
    pub rate_limit_burst: u32,
    pub rate_limit_per_sec: f64,
    pub max_text_chars: usize,
    pub max_frame_bytes: usize,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let join_token_secret =
            env::var("JOIN_TOKEN_SECRET").context("JOIN_TOKEN_SECRET is required")?;
        ensure!(
            join_token_secret.len() >= MIN_JOIN_TOKEN_SECRET_LEN,
            "JOIN_TOKEN_SECRET must be at least {MIN_JOIN_TOKEN_SECRET_LEN} characters"
        );

        Ok(Self {
            public_addr: required("GATEWAY_PUBLIC_ADDR")?,
            internal_addr: required("GATEWAY_INTERNAL_ADDR")?,
            join_token_secret,
            room: RoomConfig {
                flush_interval: Duration::from_millis(required("ROOM_FLUSH_INTERVAL_MS")?),
                max_messages_per_flush: required("ROOM_MAX_MESSAGES_PER_FLUSH")?,
                broadcast_capacity: required("ROOM_BROADCAST_CAPACITY")?,
            },
            session: SessionConfig {
                send_timeout: Duration::from_millis(required("SESSION_SEND_TIMEOUT_MS")?),
                rate_limit_burst: required("SESSION_RATE_LIMIT_BURST")?,
                rate_limit_per_sec: required("SESSION_RATE_LIMIT_PER_SEC")?,
                max_text_chars: required("SESSION_MAX_TEXT_CHARS")?,
                max_frame_bytes: required("SESSION_MAX_FRAME_BYTES")?,
            },
            store: StoreConfig {
                database_url: required("DATABASE_URL")?,
                queue_capacity: required("STORE_QUEUE_CAPACITY")?,
                batch_max_messages: required("STORE_BATCH_MAX_MESSAGES")?,
                max_connections: required("STORE_MAX_CONNECTIONS")?,
            },
            archive: ArchiveConfig {
                dir: required("ARCHIVE_DIR")?,
                flush_interval: Duration::from_secs(required("ARCHIVE_FLUSH_INTERVAL_SECS")?),
                max_messages_per_file: required("ARCHIVE_MAX_MESSAGES_PER_FILE")?,
                queue_capacity: required("ARCHIVE_QUEUE_CAPACITY")?,
            },
        })
    }
}

fn required<T>(key: &str) -> Result<T>
where
    T: FromStr,
    T::Err: Display,
{
    let raw = env::var(key).with_context(|| format!("{key} is required"))?;
    raw.parse()
        .map_err(|err| anyhow!("{key} has an invalid value {raw:?}: {err}"))
}
