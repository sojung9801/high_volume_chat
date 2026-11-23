import { createHash, randomBytes } from 'node:crypto';

import { OWNER_KEY_BYTES } from './room.constants.ts';

/** 방장 열쇠를 새로 만든다. 무작위 값이라 추측할 수 없다. */
export function generateOwnerKey(): string {
  return randomBytes(OWNER_KEY_BYTES).toString('base64url');
}

/**
 * DB에 저장할 값. 해시는 원문으로 되돌릴 수 없어서 DB가 유출돼도 열쇠는 드러나지 않는다.
 * 열쇠가 충분히 긴 무작위 값이라 SHA-256 한 번으로 충분하다 (사람이 정한 비밀번호라면 부족하다).
 */
export function hashOwnerKey(ownerKey: string): string {
  return createHash('sha256').update(ownerKey).digest('hex');
}
