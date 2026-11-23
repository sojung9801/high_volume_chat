import type { DatabaseClient } from '../db/database.types.ts';
import { EXPORT_CURSOR_ROWS } from './message.constants.ts';
import type { RoomMessage, RoomMessageRow } from './message.types.ts';
import { toRoomMessage } from './message.utils.ts';

/**
 * 방의 모든 메시지를 오래된 순으로 조금씩 꺼낸다.
 * 긴 방송은 수십만 건이 될 수 있어서 한 번에 다 읽지 않는다.
 * 조회와 마찬가지로 지금 방이 만들어진 뒤의 메시지만 내보낸다.
 */
export async function* streamRoomMessages(
  database: DatabaseClient,
  roomId: string,
): AsyncGenerator<RoomMessage[]> {
  const cursor = database<RoomMessageRow[]>`
    SELECT sent_at_ms, seq, user_id, nickname, body, is_owner
    FROM chat_messages
    WHERE room_id = ${roomId}
      AND sent_at_ms >= (
        SELECT (extract(epoch FROM created_at) * 1000)::bigint FROM rooms WHERE room_id = ${roomId}
      )
    ORDER BY sent_at_ms, seq
  `.cursor(EXPORT_CURSOR_ROWS);

  for await (const rows of cursor) {
    yield rows.map(toRoomMessage);
  }
}
