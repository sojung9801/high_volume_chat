import type { DatabaseClient } from '../db/database.types.ts';

/**
 * 방에 닫힌 시각을 기록하고 그 방의 회차를 돌려준다. 이미 닫힌 방이면 시각은 그대로 둔다.
 * 이후 이 방의 입장 토큰은 발급되지 않는다. 그런 방이 없으면 `null`.
 */
export async function markRoomClosed(
  database: DatabaseClient,
  roomId: string,
): Promise<number | null> {
  const rows = await database<{ generation: string }[]>`
    UPDATE rooms
    SET closed_at = coalesce(closed_at, now())
    WHERE room_id = ${roomId}
    RETURNING (extract(epoch FROM created_at) * 1000)::bigint AS generation
  `;
  const generation = rows[0]?.generation;
  return generation === undefined ? null : Number(generation);
}
