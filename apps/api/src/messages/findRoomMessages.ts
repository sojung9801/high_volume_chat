import type { DatabaseClient } from '../db/database.types.ts';
import type { RoomMessage, RoomMessageCursor, RoomMessageRow } from './message.types.ts';
import { toRoomMessage } from './message.utils.ts';

/**
 * 방의 메시지를 최신 쪽부터 `limit`개 읽어 오래된 순으로 돌려준다.
 * `before`가 있으면 그보다 오래된 것만 읽는다. 기본 키 인덱스를 그대로 탄다.
 *
 * 지금 방이 만들어진 뒤의 메시지만 읽는다. 같은 ID로 방을 다시 열었을 때
 * 이전 회차의 채팅이 섞이지 않게 하기 위해서다.
 */
export async function findRoomMessages(
  database: DatabaseClient,
  roomId: string,
  before: RoomMessageCursor | null,
  limit: number,
): Promise<RoomMessage[]> {
  const olderThan = before
    ? database`AND (sent_at_ms, seq) < (${before.sentAtMs}, ${before.seq})`
    : database``;

  const rows = await database<RoomMessageRow[]>`
    SELECT sent_at_ms, seq, user_id, nickname, body, is_owner
    FROM chat_messages
    WHERE room_id = ${roomId} ${olderThan}
      AND sent_at_ms >= (
        SELECT (extract(epoch FROM created_at) * 1000)::bigint FROM rooms WHERE room_id = ${roomId}
      )
    ORDER BY sent_at_ms DESC, seq DESC
    LIMIT ${limit}
  `;

  return rows.map(toRoomMessage).reverse();
}
