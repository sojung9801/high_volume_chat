import { timingSafeEqual } from 'node:crypto';

import type { DatabaseClient } from '../db/database.types.ts';
import { hashOwnerKey } from './ownerKey.ts';

/**
 * 받은 열쇠가 이 방의 방장 열쇠인지 확인한다. 닫힌 방에도 답한다 (닫은 뒤 내역을 받을 수 있어야 한다).
 * 받은 열쇠를 같은 방식으로 해시해서 저장된 해시와 비교한다.
 */
export async function isRoomOwner(
  database: DatabaseClient,
  roomId: string,
  ownerKey: string,
): Promise<boolean> {
  const rows = await database<{ owner_key_hash: string }[]>`
    SELECT owner_key_hash FROM rooms WHERE room_id = ${roomId}
  `;
  const stored = rows[0]?.owner_key_hash;
  if (stored === undefined) {
    return false;
  }

  const expected = Buffer.from(stored, 'hex');
  const actual = Buffer.from(hashOwnerKey(ownerKey), 'hex');
  // 비교에 걸리는 시간으로 값을 추측하지 못하게 일정 시간 비교를 쓴다.
  return expected.length === actual.length && timingSafeEqual(expected, actual);
}
