import { SignJWT } from 'jose';

import { JOIN_TOKEN_AUDIENCE, JOIN_TOKEN_TTL_SEC } from './room.constants.ts';
import type { RoomJoinTokenClaims } from './room.types.ts';

/** 게이트웨이가 검증하는 HS256 입장 토큰을 만든다. */
export async function signRoomJoinToken(
  claims: RoomJoinTokenClaims,
  secret: string,
): Promise<string> {
  return new SignJWT({ room: claims.roomId, nick: claims.nickname, owner: claims.isOwner })
    .setProtectedHeader({ alg: 'HS256' })
    .setSubject(claims.userId)
    .setAudience(JOIN_TOKEN_AUDIENCE)
    .setIssuedAt()
    .setExpirationTime(`${JOIN_TOKEN_TTL_SEC}s`)
    .sign(new TextEncoder().encode(secret));
}
