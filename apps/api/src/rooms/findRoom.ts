import type { DatabaseClient } from '../db/database.types.ts';
import type { RoomState } from './room.types.ts';

/**
 * 방의 현재 상태. 그런 방이 없으면 `null`.
 *
 * `isOpen`: 닫히지 않았고 유효기간도 안 지났다. 정리 작업이 돌기 전이라도 기한이 지나면 닫힌 것으로 본다.
 * `generation`: 방의 회차. 방을 만든 시각(ms)이다. 같은 ID로 다시 열면 커진다.
 */
export async function findRoom(
  database: DatabaseClient,
  roomId: string,
  roomTtlHours: number,
): Promise<RoomState | null> {
  const rows = await database<{ generation: string; is_open: boolean }[]>`
    SELECT
      (extract(epoch FROM created_at) * 1000)::bigint AS generation,
      closed_at IS NULL AND created_at > now() - make_interval(hours => ${roomTtlHours}) AS is_open
    FROM rooms
    WHERE room_id = ${roomId}
  `;
  const row = rows[0];
  return row === undefined ? null : { generation: Number(row.generation), isOpen: row.is_open };
}
