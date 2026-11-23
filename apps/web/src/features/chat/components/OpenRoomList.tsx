'use client';

import { useTranslations } from 'next-intl';

import { useOpenRooms } from '../hooks/useOpenRooms';
import styles from './ChatRoom.module.css';

type OpenRoomListProps = {
  selectedRoomId: string;
  /** 방을 고르면 그 ID를 알려준다. 입장 폼의 방 ID 칸을 채우는 데 쓴다. */
  onSelect: (roomId: string) => void;
};

export function OpenRoomList({ selectedRoomId, onSelect }: OpenRoomListProps) {
  const t = useTranslations();
  const rooms = useOpenRooms();

  return (
    <section className={styles.openRooms}>
      <h2 className={styles.openRoomsTitle}>{t('messages.chat.open_rooms')}</h2>
      {rooms === null ? null : rooms.length === 0 ? (
        <p className={styles.openRoomsEmpty}>{t('messages.chat.no_open_rooms')}</p>
      ) : (
        <ul className={styles.openRoomsList}>
          {rooms.map((room) => (
            <li key={room.roomId}>
              <button
                type="button"
                className={styles.openRoomButton}
                aria-pressed={room.roomId === selectedRoomId}
                onClick={() => onSelect(room.roomId)}
              >
                {room.roomId}
              </button>
            </li>
          ))}
        </ul>
      )}
    </section>
  );
}
