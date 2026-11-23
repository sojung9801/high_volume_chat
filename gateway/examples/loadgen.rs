//! 떠 있는 게이트웨이의 방에 봇을 붙여 메시지를 쏟아붓는다. 브라우저로 같은 방에 들어가 화면을 본다.
//!
//!   cargo run --release --example loadgen -- <방 ID> <봇 수> <초>
//!
//! 방은 먼저 만들어져 있어야 한다 (브라우저에서 "방 만들기"). 관리 API에 방의 회차를 물어본 뒤
//! `.env`의 `JOIN_TOKEN_SECRET`으로 입장 토큰을 직접 서명하고 `GATEWAY_PUBLIC_WS_URL`로 접속한다.
//! 봇도 실제 사용자와 같은 rate limit을 받는다.

use std::{
    env,
    str::FromStr,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result, anyhow};
use chat_gateway::proto::chat::v1::{
    ClientFrame, SendMessage, ServerFrame, client_frame, server_frame,
};
use futures_util::{SinkExt, StreamExt, future::join_all};
use jsonwebtoken::{Algorithm, EncodingKey, Header, encode};
use prost::Message as _;
use serde::{Deserialize, Serialize};
use tokio_tungstenite::{connect_async, tungstenite::Message};

const JOIN_TOKEN_AUDIENCE: &str = "chat-gateway";
const JOIN_TOKEN_TTL_SEC: u64 = 60;
const SEND_INTERVAL: Duration = Duration::from_secs(1);
const CONNECT_CHUNK: usize = 100;
const USAGE: &str = "usage: loadgen <room-id> <bots> <seconds>";

#[derive(Serialize)]
struct JoinTokenClaims<'a> {
    sub: &'a str,
    room: &'a str,
    nick: &'a str,
    /// 방의 회차. 게이트웨이는 닫힌 회차나 지난 회차의 토큰을 받지 않는다.
    #[serde(rename = "gen")]
    generation: u64,
    aud: &'a str,
    exp: u64,
}

#[derive(Default)]
struct Counters {
    sent: AtomicU64,
    rate_limited: AtomicU64,
    failed_connections: AtomicU64,
}

fn parse_arg<T: FromStr>(args: &[String], index: usize, name: &str) -> Result<T> {
    let raw = args.get(index).ok_or_else(|| anyhow!("{name} is missing\n{USAGE}"))?;
    raw.parse()
        .map_err(|_| anyhow!("{name} has an invalid value {raw:?}\n{USAGE}"))
}

#[derive(Deserialize)]
struct OpenRoom {
    generation: u64,
}

/// 관리 API에 방이 열려 있는지 묻고 회차를 받는다.
async fn fetch_room_generation(api_port: &str, room_id: &str) -> Result<u64> {
    let url = format!("http://127.0.0.1:{api_port}/v1/rooms/{room_id}");
    let response = reqwest::get(&url)
        .await
        .with_context(|| format!("cannot reach the API at {url}"))?;
    if response.status() == reqwest::StatusCode::NOT_FOUND {
        return Err(anyhow!("room {room_id:?} is not open. Create it in the browser first"));
    }
    let room: OpenRoom = response.error_for_status()?.json().await?;
    Ok(room.generation)
}

fn sign_join_token(
    secret: &str,
    room_id: &str,
    bot_name: &str,
    generation: u64,
) -> Result<String> {
    let exp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs() + JOIN_TOKEN_TTL_SEC;
    let claims = JoinTokenClaims {
        sub: bot_name,
        room: room_id,
        nick: bot_name,
        generation,
        aud: JOIN_TOKEN_AUDIENCE,
        exp,
    };
    let key = EncodingKey::from_secret(secret.as_bytes())?;
    Ok(encode(&Header::new(Algorithm::HS256), &claims, &key)?)
}

fn encode_send_message(text: String) -> Message {
    let frame = ClientFrame {
        payload: Some(client_frame::Payload::SendMessage(SendMessage { text })),
    };
    Message::Binary(frame.encode_to_vec().into())
}

async fn run_bot(
    url: String,
    bot_name: String,
    start_delay: Duration,
    deadline: Instant,
    counters: Arc<Counters>,
) {
    let Ok((socket, _)) = connect_async(url).await else {
        counters.failed_connections.fetch_add(1, Ordering::Relaxed);
        return;
    };
    let (mut sink, mut stream) = socket.split();

    // 방 프레임을 읽어서 버린다. rate limit 오류만 센다.
    let reader_counters = Arc::clone(&counters);
    let reader = tokio::spawn(async move {
        while let Some(Ok(message)) = stream.next().await {
            let Message::Binary(data) = message else { continue };
            let Ok(frame) = ServerFrame::decode(data.as_ref()) else { continue };
            if let Some(server_frame::Payload::Error(_)) = frame.payload {
                reader_counters.rate_limited.fetch_add(1, Ordering::Relaxed);
            }
        }
    });

    // 봇마다 시작 시점을 흩어서 1초 안에 고르게 들어오게 한다.
    tokio::time::sleep(start_delay).await;
    let mut ticker = tokio::time::interval(SEND_INTERVAL);
    let mut count = 0u64;
    while Instant::now() < deadline {
        ticker.tick().await;
        count += 1;
        if sink
            .send(encode_send_message(format!("{bot_name} 메시지 #{count}")))
            .await
            .is_err()
        {
            break;
        }
        counters.sent.fetch_add(1, Ordering::Relaxed);
    }

    let _ = sink.close().await;
    reader.abort();
}

#[tokio::main]
async fn main() -> Result<()> {
    dotenvy::dotenv().ok();
    let args: Vec<String> = env::args().skip(1).collect();
    let room_id: String = parse_arg(&args, 0, "room-id")?;
    let bots: usize = parse_arg(&args, 1, "bots")?;
    let seconds: u64 = parse_arg(&args, 2, "seconds")?;

    let secret = env::var("JOIN_TOKEN_SECRET").context("JOIN_TOKEN_SECRET is required")?;
    let ws_url = env::var("GATEWAY_PUBLIC_WS_URL").context("GATEWAY_PUBLIC_WS_URL is required")?;
    let api_port = env::var("API_PORT").context("API_PORT is required")?;
    let generation = fetch_room_generation(&api_port, &room_id).await?;

    let counters = Arc::new(Counters::default());
    let deadline = Instant::now() + Duration::from_secs(seconds);
    println!("room {room_id}: {bots} bots for {seconds}s -> {ws_url}");

    let mut tasks = Vec::with_capacity(bots);
    for index in 0..bots {
        let bot_name = format!("bot-{index}");
        let token = sign_join_token(&secret, &room_id, &bot_name, generation)?;
        let start_delay = SEND_INTERVAL.mul_f64(index as f64 / bots as f64);
        tasks.push(tokio::spawn(run_bot(
            format!("{ws_url}?token={token}"),
            bot_name,
            start_delay,
            deadline,
            Arc::clone(&counters),
        )));
        // 한꺼번에 열면 listen backlog가 넘친다.
        if (index + 1) % CONNECT_CHUNK == 0 {
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    let progress_counters = Arc::clone(&counters);
    let progress = tokio::spawn(async move {
        let mut ticker = tokio::time::interval(Duration::from_secs(1));
        let mut previous = 0;
        loop {
            ticker.tick().await;
            let sent = progress_counters.sent.load(Ordering::Relaxed);
            println!("sent {sent} (+{}/s)", sent - previous);
            previous = sent;
        }
    });

    join_all(tasks).await;
    progress.abort();

    println!("--- loadgen done ---");
    println!("sent               {}", counters.sent.load(Ordering::Relaxed));
    println!("rate limited       {}", counters.rate_limited.load(Ordering::Relaxed));
    println!("failed connections {}", counters.failed_connections.load(Ordering::Relaxed));
    Ok(())
}
