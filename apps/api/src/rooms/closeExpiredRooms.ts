import type { ApiEnv } from '../config/env.types.ts';
import type { DatabaseClient } from '../db/database.types.ts';
import { closeGatewayRoom } from '../gateway/gatewayInternalClient.ts';

/**
 * 유효기간이 지난 방을 닫고, 아직 붙어 있는 사람을 내보낸다. 닫은 방 ID를 돌려준다.
 * 방장이 직접 닫을 때와 같은 순서다: 먼저 DB에 기록해 새 입장을 막고, 그다음 내보낸다.
 */
export async function closeExpiredRooms(database: DatabaseClient, env: ApiEnv): Promise<string[]> {
  const expired = await database<{ room_id: string; generation: string }[]>`
    UPDATE rooms
    SET closed_at = now()
    WHERE closed_at IS NULL
      AND created_at <= now() - make_interval(hours => ${env.ROOM_TTL_HOURS})
    RETURNING room_id, (extract(epoch FROM created_at) * 1000)::bigint AS generation
  `;

  const roomIds = expired.map((row) => row.room_id);
  for (const { room_id: roomId, generation } of expired) {
    try {
      await closeGatewayRoom(env.GATEWAY_INTERNAL_URL, roomId, Number(generation));
    } catch (error) {
      // 방은 이미 닫혔다. 남은 연결은 끊기는 대로 다시 들어오지 못한다.
      console.error(`room ${roomId} expired but gateway could not be told`, error);
    }
  }
  return roomIds;
}
