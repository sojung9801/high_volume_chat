import type { DatabaseClient } from '../db/database.types.ts';

/**
 * 방을 만든다. 같은 ID의 방이 열려 있으면 아무것도 하지 않고 `false`를 돌려준다.
 * 같은 ID가 있더라도 이미 닫혔거나 유효기간이 지난 방이면 새 방으로 다시 연다
 * (방장 열쇠와 만든 시각을 새로 쓴다).
 *
 * "있는지 조회 → 없으면 저장"으로 나누지 않는다. 그 사이에 다른 요청이 끼어들 수 있다.
 * 한 문장으로 DB가 판단하게 해서, 동시에 요청이 와도 한 명만 성공한다.
 */
export async function insertRoom(
  database: DatabaseClient,
  roomId: string,
  ownerKeyHash: string,
  roomTtlHours: number,
): Promise<boolean> {
  const opened = await database`
    INSERT INTO rooms (room_id, owner_key_hash)
    VALUES (${roomId}, ${ownerKeyHash})
    ON CONFLICT (room_id) DO UPDATE
      SET owner_key_hash = EXCLUDED.owner_key_hash,
          created_at = now(),
          closed_at = NULL
      WHERE rooms.closed_at IS NOT NULL
         OR rooms.created_at <= now() - make_interval(hours => ${roomTtlHours})
    RETURNING room_id
  `;
  return opened.length === 1;
}
