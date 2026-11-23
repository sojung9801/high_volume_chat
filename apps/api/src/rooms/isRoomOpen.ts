import type { DatabaseClient } from '../db/database.types.ts';

/**
 * 만들어졌고, 닫히지 않았고, 유효기간도 안 지난 방인지 확인한다.
 * 유효기간은 만든 시각부터 센다. 정리 작업이 돌기 전이라도 기한이 지나면 바로 닫힌 것으로 본다.
 */
export async function isRoomOpen(
  database: DatabaseClient,
  roomId: string,
  roomTtlHours: number,
): Promise<boolean> {
  const rows = await database`
    SELECT 1 FROM rooms
    WHERE room_id = ${roomId}
      AND closed_at IS NULL
      AND created_at > now() - make_interval(hours => ${roomTtlHours})
  `;
  return rows.length === 1;
}
