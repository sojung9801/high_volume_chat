import type { DatabaseClient } from '../db/database.types.ts';

/** 방에 닫힌 시각을 기록한다. 이미 닫힌 방이면 그대로 둔다. 이후 이 방의 입장 토큰은 발급되지 않는다. */
export async function markRoomClosed(database: DatabaseClient, roomId: string): Promise<void> {
  await database`
    UPDATE rooms SET closed_at = now() WHERE room_id = ${roomId} AND closed_at IS NULL
  `;
}
