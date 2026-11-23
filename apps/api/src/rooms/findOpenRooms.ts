import type { DatabaseClient } from '../db/database.types.ts';
import { OPEN_ROOM_LIST_LIMIT } from './room.constants.ts';
import type { OpenRoom, OpenRoomRow } from './room.types.ts';

/** 닫히지 않았고 유효기간도 안 지난 방을 최근에 만든 순으로 돌려준다. */
export async function findOpenRooms(
  database: DatabaseClient,
  roomTtlHours: number,
): Promise<OpenRoom[]> {
  const rows = await database<OpenRoomRow[]>`
    SELECT room_id, created_at
    FROM rooms
    WHERE closed_at IS NULL
      AND created_at > now() - make_interval(hours => ${roomTtlHours})
    ORDER BY created_at DESC
    LIMIT ${OPEN_ROOM_LIST_LIMIT}
  `;
  return rows.map((row) => ({ roomId: row.room_id, createdAt: row.created_at.toISOString() }));
}
