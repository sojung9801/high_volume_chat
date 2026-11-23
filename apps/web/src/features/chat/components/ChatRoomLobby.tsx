'use client';

import { useState } from 'react';

import { ChatRoomEntryForm } from './ChatRoomEntryForm';
import { OpenRoomList } from './OpenRoomList';
import styles from './ChatRoom.module.css';

/** 첫 화면. 왼쪽에 입장 폼, 오른쪽에 진행 중인 방 목록. */
export function ChatRoomLobby() {
  // 목록에서 고른 방이 폼의 방 ID 칸에 들어가야 해서 둘의 부모가 값을 갖는다.
  const [roomId, setRoomId] = useState('');

  return (
    <main className={styles.lobby}>
      <ChatRoomEntryForm roomId={roomId} onRoomIdChange={setRoomId} />
      <OpenRoomList selectedRoomId={roomId} onSelect={setRoomId} />
    </main>
  );
}
