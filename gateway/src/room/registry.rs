use std::sync::Arc;

use bytes::Bytes;
use dashmap::DashMap;
use tokio::sync::broadcast::{self, error::RecvError};

use crate::{archive::MessageArchive, config::RoomConfig, store::MessageStore};

use super::room::{Room, RoomStats};

pub struct RoomRegistry {
    rooms: DashMap<String, Arc<Room>>,
    config: RoomConfig,
    store: MessageStore,
    archive: MessageArchive,
}

impl RoomRegistry {
    pub fn new(config: RoomConfig, store: MessageStore, archive: MessageArchive) -> Self {
        Self {
            rooms: DashMap::new(),
            config,
            store,
            archive,
        }
    }

    /// 방에 입장한다. 방이 없으면 만든다.
    ///
    /// 구독은 맵 엔트리 락을 쥔 채로 한다. 그래야 마지막 퇴장자의 방 제거와
    /// 겹쳐도 레지스트리에서 빠진 방을 구독하는 일이 없다.
    pub fn join(self: &Arc<Self>, room_id: &str) -> RoomMembership {
        let entry = self
            .rooms
            .entry(room_id.to_owned())
            .or_insert_with(|| {
                Room::spawn(
                    room_id.to_owned(),
                    self.config,
                    self.store.clone(),
                    self.archive.clone(),
                )
            });
        let frames = entry.subscribe();
        let room = Arc::clone(entry.value());
        drop(entry);

        RoomMembership {
            registry: Arc::clone(self),
            room,
            frames: Some(frames),
        }
    }

    /// 방을 닫는다. 그런 방이 떠 있지 않으면 `false`.
    pub fn close(&self, room_id: &str) -> bool {
        match self.rooms.get(room_id) {
            Some(room) => {
                room.close();
                true
            }
            None => false,
        }
    }

    pub fn stats(&self) -> Vec<RoomStats> {
        self.rooms.iter().map(|entry| entry.value().stats()).collect()
    }

    fn remove_if_empty(&self, room_id: &str) {
        self.rooms
            .remove_if(room_id, |_, room| room.subscriber_count() == 0);
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
        self.registry.remove_if_empty(self.room.id());
    }
}
