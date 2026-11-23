use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use bytes::Bytes;
use dashmap::{DashMap, mapref::entry::Entry};
use tokio::sync::broadcast::{self, error::RecvError};

use crate::{archive::MessageArchive, config::RoomConfig, store::MessageStore};

use super::room::{Room, RoomStats};

/// 닫힌 방을 기억해 두는 시간. 입장 토큰 수명(60초)보다 길어야 한다.
/// 이 시간이 지나면 닫히기 전에 발급된 토큰은 어차피 만료되어 서명 검증에서 걸린다.
const CLOSED_ROOM_RETENTION: Duration = Duration::from_secs(180);

struct ClosedRoom {
    /// 이 회차까지 닫혔다. 이보다 큰 회차(다시 연 방)의 토큰만 들어올 수 있다.
    generation: u64,
    closed_at: Instant,
}

pub struct RoomRegistry {
    rooms: DashMap<String, Arc<Room>>,
    /// 최근에 닫힌 방. 마지막 사람이 나가 방이 사라진 뒤에도 "닫혔다"는 사실을 잊지 않기 위해 따로 둔다.
    closed: DashMap<String, ClosedRoom>,
    config: RoomConfig,
    store: MessageStore,
    archive: MessageArchive,
}

impl RoomRegistry {
    pub fn new(config: RoomConfig, store: MessageStore, archive: MessageArchive) -> Self {
        Self {
            rooms: DashMap::new(),
            closed: DashMap::new(),
            config,
            store,
            archive,
        }
    }

    /// 방에 입장한다. 방이 없으면 만든다. 들어올 수 없는 토큰이면 `None`.
    ///
    /// 거절하는 경우:
    /// - 닫힌 회차(또는 그 이전)의 토큰. 방이 닫힌 뒤에도 토큰은 60초 동안 유효하기 때문에 여기서 막는다.
    /// - 지금 떠 있는 방보다 이전 회차의 토큰.
    ///
    /// 지금 떠 있는 방보다 **새** 회차의 토큰이 오면, 같은 ID로 방이 다시 열린 것이다.
    /// 이전 회차의 방을 닫아 남아 있던 사람을 내보내고 새 방을 만든다.
    ///
    /// 구독은 맵 엔트리 락을 쥔 채로 한다. 그래야 마지막 퇴장자의 방 제거와
    /// 겹쳐도 레지스트리에서 빠진 방을 구독하는 일이 없다.
    pub fn join(self: &Arc<Self>, room_id: &str, generation: Option<u64>) -> Option<RoomMembership> {
        if let Some(closed) = self.closed.get(room_id) {
            // 회차 없는 토큰은 닫힌 기록이 있는 동안 받지 않는다.
            if generation.is_none_or(|token| token <= closed.generation) {
                return None;
            }
        }

        let (room, frames) = match self.rooms.entry(room_id.to_owned()) {
            Entry::Occupied(mut entry) => {
                if let (Some(token), Some(live)) = (generation, entry.get().generation()) {
                    if token < live {
                        return None;
                    }
                    if token > live {
                        entry.get().close();
                        entry.insert(self.spawn_room(room_id, generation));
                    }
                }
                (Arc::clone(entry.get()), entry.get().subscribe())
            }
            Entry::Vacant(entry) => {
                let room = entry.insert(self.spawn_room(room_id, generation));
                (Arc::clone(room.value()), room.subscribe())
            }
        };

        Some(RoomMembership {
            registry: Arc::clone(self),
            room,
            frames: Some(frames),
        })
    }

    /// `generation` 회차까지의 방을 닫는다. 떠 있던 방을 닫았으면 `true`.
    ///
    /// 떠 있는 방이 없어도 닫힌 사실은 기록한다. 그래야 닫히기 직전에 발급된 토큰으로
    /// 뒤늦게 들어와 방을 되살리지 못한다.
    pub fn close(&self, room_id: &str, generation: u64) -> bool {
        self.closed
            .retain(|_, closed| closed.closed_at.elapsed() < CLOSED_ROOM_RETENTION);
        self.closed.insert(
            room_id.to_owned(),
            ClosedRoom {
                generation,
                closed_at: Instant::now(),
            },
        );

        match self.rooms.get(room_id) {
            // 더 새 회차의 방이 이미 떠 있으면 건드리지 않는다.
            Some(room) if room.generation().is_none_or(|live| live <= generation) => {
                room.close();
                true
            }
            _ => false,
        }
    }

    pub fn stats(&self) -> Vec<RoomStats> {
        self.rooms.iter().map(|entry| entry.value().stats()).collect()
    }

    fn spawn_room(&self, room_id: &str, generation: Option<u64>) -> Arc<Room> {
        Room::spawn(
            room_id.to_owned(),
            generation,
            self.config,
            self.store.clone(),
            self.archive.clone(),
        )
    }

    /// 비어 있으면 지운다. 같은 ID로 새 회차의 방이 들어와 있을 수 있으므로 자기 방일 때만 지운다.
    fn remove_if_empty(&self, room: &Arc<Room>) {
        self.rooms.remove_if(room.id(), |_, current| {
            Arc::ptr_eq(current, room) && current.subscriber_count() == 0
        });
    }
}

/// 한 연결의 방 입장 상태. drop되면 구독을 풀고, 빈 방이면 레지스트리에서 지운다.
pub struct RoomMembership {
    registry: Arc<RoomRegistry>,
    room: Arc<Room>,
    frames: Option<broadcast::Receiver<Bytes>>,
}

impl RoomMembership {
    pub fn room(&self) -> Arc<Room> {
        Arc::clone(&self.room)
    }

    pub async fn recv(&mut self) -> Result<Bytes, RecvError> {
        match self.frames.as_mut() {
            Some(frames) => frames.recv().await,
            None => Err(RecvError::Closed),
        }
    }
}

impl Drop for RoomMembership {
    fn drop(&mut self) {
        // 구독을 먼저 풀어야 구독자 수에 이 연결이 빠진다.
        drop(self.frames.take());
        self.registry.remove_if_empty(&self.room);
    }
}
